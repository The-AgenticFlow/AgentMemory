//! Minimal MCP-compatible JSON-RPC surface for Agent Memory.
//!
//! This implements the core methods agents need (`initialize`, tools,
//! resources, prompts) over Streamable HTTP at `/mcp` and over stdio.

use std::collections::HashMap;

use axum::{extract::State, Json};
use engram_core::{BankType, DispositionConfig, MemoryBank, SessionMode, WorkingContext};
use engram_runtime::{RetrievalOutcome, RuntimeConfig, SessionHandle};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use uuid::Uuid;

use crate::routes::AppState;

pub async fn mcp_http(State(state): State<AppState>, Json(request): Json<Value>) -> Json<Value> {
    Json(handle_json_rpc(&state, request).await)
}

pub async fn run_stdio(state: AppState) -> anyhow::Result<()> {
    let stdin = BufReader::new(tokio::io::stdin());
    let mut lines = stdin.lines();
    let mut stdout = tokio::io::stdout();

    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = serde_json::from_str(&line)
            .unwrap_or_else(|error| json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":error.to_string()}}));
        let response = handle_json_rpc(&state, request).await;
        stdout.write_all(serde_json::to_string(&response)?.as_bytes()).await?;
        stdout.write_all(b"\n").await?;
        stdout.flush().await?;
    }

    Ok(())
}

async fn handle_json_rpc(state: &AppState, request: Value) -> Value {
    if let Some(batch) = request.as_array() {
        let mut responses = Vec::with_capacity(batch.len());
        for item in batch {
            responses.push(handle_one(state, item.clone()).await);
        }
        return Value::Array(responses);
    }
    handle_one(state, request).await
}

async fn handle_one(state: &AppState, request: Value) -> Value {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    let params = request.get("params").cloned().unwrap_or_else(|| json!({}));

    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": "2025-06-18",
            "serverInfo": { "name": "agent-memory", "version": env!("CARGO_PKG_VERSION") },
            "capabilities": {
                "tools": {},
                "resources": {},
                "prompts": {}
            }
        })),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "tools/call" => call_tool(state, params).await,
        "resources/list" => Ok(json!({ "resources": resources() })),
        "resources/read" => read_resource(state, params).await,
        "prompts/list" => Ok(json!({ "prompts": prompts() })),
        "prompts/get" => get_prompt(params).await,
        _ => Err((-32601, format!("unknown MCP method: {method}"))),
    };

    match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, message)) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }),
    }
}

async fn call_tool(state: &AppState, params: Value) -> Result<Value, (i64, String)> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| (-32602, "tools/call requires params.name".to_string()))?;
    let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));

    let value = match name {
        "memory_open_session" => {
            let expectation = string_arg(&args, "expectation", "remember useful context");
            let task_context = string_arg(&args, "task_context", "agent task");
            let mode = parse_mode(args.get("mode").and_then(Value::as_str).unwrap_or("Exploration"))?;
            let bank_id = args.get("bank_id").and_then(Value::as_str).and_then(|s| Uuid::parse_str(s).ok());
            let handle = state
                .system
                .open_session(None, bank_id, expectation, mode, task_context)
                .await
                .map_err(internal)?;
            state.sessions.write().await.insert(handle.session.id, handle.clone());
            serde_json::to_value(handle).map_err(internal)?
        }
        "memory_close_session" => {
            let session_id = uuid_arg(&args, "session_id")?;
            let mut handle = if let Some(h) = state.sessions.write().await.remove(&session_id) {
                h
            } else {
                let session = state.system.postgres.get_session(session_id).await.map_err(internal)?
                    .ok_or_else(|| (-32004, format!("session not found: {session_id}")))?;
                let ctx = state.system.postgres.get_working_context(session_id).await.map_err(internal)?;
                SessionHandle { session, working_context: ctx }
            };
            state.system.close_session(&mut handle).await.map_err(internal)?;
            json!({ "closed": true, "session_id": session_id })
        }
        "memory_create_bank" => {
            let bank_name = string_arg(&args, "name", "new-bank");
            let bank_type = parse_bank_type(args.get("type").and_then(Value::as_str).unwrap_or("dictionary"))?;
            let mission = args.get("mission").and_then(Value::as_str).map(|s| s.to_string());
            let directives: Vec<String> = args
                .get("directives")
                .and_then(Value::as_array)
                .map(|arr| arr.iter().filter_map(Value::as_str).map(|s| s.to_string()).collect())
                .unwrap_or_default();
            let parent_bank_id = args.get("parent_bank_id").and_then(Value::as_str).and_then(|s| Uuid::parse_str(s).ok());
            let disposition = DispositionConfig::default();
            let bank = MemoryBank::new(bank_name, None, bank_type, mission, directives, disposition, parent_bank_id);
            state.system.postgres.save_bank(&bank).await.map_err(internal)?;
            serde_json::to_value(bank).map_err(internal)?
        }
        "memory_retain" => {
            let session_id = uuid_arg(&args, "session_id")?;
            let mut handle = load_handle(state, session_id).await?;
            let outcome = state
                .system
                .process_episode(
                    &mut handle,
                    string_arg(&args, "action", ""),
                    string_arg(&args, "context", ""),
                    string_arg(&args, "outcome", ""),
                )
                .await
                .map_err(internal)?;
            state.sessions.write().await.insert(session_id, handle);
            serde_json::to_value(outcome).map_err(internal)?
        }
        "memory_recall" => {
            let session_id = uuid_arg(&args, "session_id")?;
            let handle = load_handle(state, session_id).await?;
            let retrieval: RetrievalOutcome = state
                .system
                .retrieve(&handle, string_arg(&args, "query", ""))
                .await
                .map_err(internal)?;
            state.sessions.write().await.insert(session_id, handle);
            serde_json::to_value(retrieval).map_err(internal)?
        }
        "memory_reflect" => {
            let schemas = state.system.consolidate().await.map_err(internal)?;
            json!({ "created_schemas": schemas.len(), "schemas": schemas })
        }
        "memory_get_working_context" => {
            let session_id = uuid_arg(&args, "session_id")?;
            let handle = load_handle(state, session_id).await?;
            serde_json::to_value(handle.working_context).map_err(internal)?
        }
        "memory_update_working_context" => {
            let session_id = uuid_arg(&args, "session_id")?;
            let mut handle = load_handle(state, session_id).await?;
            let task_id = string_arg(&args, "task_id", "mcp-task");
            state
                .system
                .open_working_context(&mut handle, task_id)
                .await
                .map_err(internal)?;
            let context = handle.working_context.clone();
            state.sessions.write().await.insert(session_id, handle);
            serde_json::to_value(context).map_err(internal)?
        }
        "memory_get_config" => serde_json::to_value(state.system.runtime_config()).map_err(internal)?,
        "memory_update_config" => {
            let config: RuntimeConfig = serde_json::from_value(args).map_err(internal)?;
            let config = state
                .system
                .update_config("mcp", config)
                .await
                .map_err(internal)?;
            serde_json::to_value(config).map_err(internal)?
        }
        _ => return Err((-32602, format!("unknown tool: {name}"))),
    };

    Ok(json!({
        "content": [{ "type": "text", "text": serde_json::to_string_pretty(&value).map_err(internal)? }],
        "structuredContent": value
    }))
}

async fn read_resource(state: &AppState, params: Value) -> Result<Value, (i64, String)> {
    let uri = params
        .get("uri")
        .and_then(Value::as_str)
        .ok_or_else(|| (-32602, "resources/read requires params.uri".to_string()))?;
    let value = if uri == "engram://overview" {
        serde_json::to_value(state.system.overview(None).await.map_err(internal)?).map_err(internal)?
    } else if uri == "engram://graph" {
        serde_json::to_value(state.system.control_graph(None).await.map_err(internal)?).map_err(internal)?
    } else if let Some(id) = uri.strip_prefix("engram://sessions/") {
        let session_id = Uuid::parse_str(id).map_err(internal)?;
        let handle = load_handle(state, session_id).await?;
        serde_json::to_value(handle).map_err(internal)?
    } else if let Some(id) = uri.strip_prefix("engram://engrams/") {
        let engram_id = Uuid::parse_str(id).map_err(internal)?;
        serde_json::to_value(state.system.qdrant.get_engram(engram_id).await.map_err(internal)?)
            .map_err(internal)?
    } else if let Some(id) = uri.strip_prefix("engram://schemas/") {
        let schema_id = Uuid::parse_str(id).map_err(internal)?;
        let schema = state
            .system
            .postgres
            .list_schemas()
            .await
            .map_err(internal)?
            .into_iter()
            .find(|schema| schema.id == schema_id);
        serde_json::to_value(schema).map_err(internal)?
    } else {
        return Err((-32602, format!("unknown resource uri: {uri}")));
    };
    Ok(json!({
        "contents": [{
            "uri": uri,
            "mimeType": "application/json",
            "text": serde_json::to_string_pretty(&value).map_err(internal)?
        }]
    }))
}

async fn get_prompt(params: Value) -> Result<Value, (i64, String)> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| (-32602, "prompts/get requires params.name".to_string()))?;
    let text = match name {
        "memory-grounded-answer" => {
            "Use retrieved Agent Memory facts as numbered evidence. If memory is missing, state the gap plainly."
        }
        "session-summary" => {
            "Summarize the active session, current expectation, working context, retrieved engrams, and unresolved gaps."
        }
        "consolidation-review" => {
            "Review new schemas and weakened/archived engrams. Identify useful patterns, drift, and tuning changes."
        }
        _ => return Err((-32602, format!("unknown prompt: {name}"))),
    };
    Ok(json!({
        "description": name,
        "messages": [{
            "role": "user",
            "content": { "type": "text", "text": text }
        }]
    }))
}

fn tools() -> Vec<Value> {
    vec![
        json!({
            "name": "memory_open_session",
            "description": "Open a memory session. Optionally pass bank_id to target a specific memory bank.",
            "inputSchema": { "type": "object", "properties": {
                "expectation": { "type": "string" },
                "task_context": { "type": "string" },
                "mode": { "type": "string", "enum": ["Exploration", "Routine", "Critical", "Analogy", "Validation"] },
                "bank_id": { "type": "string" }
            }}
        }),
        json!({
            "name": "memory_close_session",
            "description": "Close an active memory session and persist its state.",
            "inputSchema": { "type": "object", "properties": {
                "session_id": { "type": "string" }
            }, "required": ["session_id"] }
        }),
        json!({
            "name": "memory_retain",
            "description": "Retain one action/context/outcome episode (Cogniti alias for capture_episode).",
            "inputSchema": { "type": "object" }
        }),
        json!({
            "name": "memory_recall",
            "description": "Recall structured memories for a query (Cogniti alias for retrieve).",
            "inputSchema": { "type": "object" }
        }),
        json!({
            "name": "memory_reflect",
            "description": "Run reflection/consolidation and schema generation (Cogniti alias for consolidate).",
            "inputSchema": { "type": "object" }
        }),
        json!({
            "name": "memory_get_working_context",
            "description": "Read the active working context",
            "inputSchema": { "type": "object" }
        }),
        json!({
            "name": "memory_update_working_context",
            "description": "Open/update a working context",
            "inputSchema": { "type": "object" }
        }),
        json!({
            "name": "memory_get_config",
            "description": "Read runtime behavior config",
            "inputSchema": { "type": "object" }
        }),
        json!({
            "name": "memory_update_config",
            "description": "Update runtime behavior config",
            "inputSchema": { "type": "object" }
        }),
        json!({
            "name": "memory_create_bank",
            "description": "Create a new hierarchical memory bank for isolated agent memory.",
            "inputSchema": { "type": "object", "properties": {
                "name": { "type": "string" },
                "type": { "type": "string", "enum": ["session", "dictionary", "shared"] },
                "mission": { "type": "string" },
                "directives": { "type": "array", "items": { "type": "string" } },
                "parent_bank_id": { "type": "string" }
            }, "required": ["name", "type"] }
        }),
    ]
}

fn resources() -> Vec<Value> {
    [
        ("engram://overview", "Agent Memory overview"),
        ("engram://graph", "Memory graph projection"),
        ("engram://sessions/{id}", "Session detail"),
        ("engram://engrams/{id}", "Engram detail"),
        ("engram://schemas/{id}", "Schema detail"),
    ]
    .into_iter()
    .map(|(uri, name)| json!({ "uri": uri, "name": name, "mimeType": "application/json" }))
    .collect()
}

fn prompts() -> Vec<Value> {
    [
        ("memory-grounded-answer", "Answer using Agent Memory evidence"),
        ("session-summary", "Summarize active session memory"),
        ("consolidation-review", "Review consolidation output"),
    ]
    .into_iter()
    .map(|(name, description)| json!({ "name": name, "description": description }))
    .collect()
}

async fn load_handle(state: &AppState, session_id: Uuid) -> Result<SessionHandle, (i64, String)> {
    if let Some(handle) = state.sessions.read().await.get(&session_id).cloned() {
        return Ok(handle);
    }
    let session = state
        .system
        .postgres
        .get_session(session_id)
        .await
        .map_err(internal)?
        .ok_or_else(|| (-32004, format!("session not found: {session_id}")))?;
    let working_context: Option<WorkingContext> = state
        .system
        .postgres
        .get_working_context(session_id)
        .await
        .map_err(internal)?;
    Ok(SessionHandle {
        session,
        working_context,
    })
}

fn parse_mode(value: &str) -> Result<SessionMode, (i64, String)> {
    match value.to_ascii_lowercase().as_str() {
        "exploration" => Ok(SessionMode::Exploration),
        "routine" => Ok(SessionMode::Routine),
        "critical" => Ok(SessionMode::Critical),
        "analogy" => Ok(SessionMode::Analogy),
        "validation" => Ok(SessionMode::Validation),
        other => Err((-32602, format!("unknown session mode: {other}"))),
    }
}

fn parse_bank_type(value: &str) -> Result<BankType, (i64, String)> {
    match value.to_ascii_lowercase().as_str() {
        "session" => Ok(BankType::Session),
        "dictionary" => Ok(BankType::Dictionary),
        "shared" => Ok(BankType::Shared),
        other => Err((-32602, format!("unknown bank type: {other}"))),
    }
}

fn uuid_arg(args: &Value, name: &str) -> Result<Uuid, (i64, String)> {
    let value = args
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| (-32602, format!("missing {name}")))?;
    Uuid::parse_str(value).map_err(internal)
}

fn string_arg(args: &Value, name: &str, default: &str) -> String {
    args.get(name)
        .and_then(Value::as_str)
        .unwrap_or(default)
        .to_string()
}

fn internal(error: impl std::fmt::Display) -> (i64, String) {
    (-32000, error.to_string())
}

#[allow(dead_code)]
fn _map(_value: HashMap<String, Value>) {}
