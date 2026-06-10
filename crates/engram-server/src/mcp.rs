//! Minimal MCP-compatible JSON-RPC surface for Agent Memory.
//!
//! This implements the core methods agents need (`initialize`, tools,
//! resources, prompts) over Streamable HTTP at `/mcp` and over stdio.

use std::collections::HashMap;

use axum::{extract::State, Json};
use engram_core::{SessionMode, WorkingContext};
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
            let handle = state
                .system
                .open_session(None, expectation, mode, task_context)
                .await
                .map_err(internal)?;
            state.sessions.write().await.insert(handle.session.id, handle.clone());
            serde_json::to_value(handle).map_err(internal)?
        }
        "memory_capture_episode" => {
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
        "memory_retrieve" => {
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
        "memory_consolidate" => {
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
        serde_json::to_value(state.system.overview().await.map_err(internal)?).map_err(internal)?
    } else if uri == "engram://graph" {
        serde_json::to_value(state.system.control_graph().await.map_err(internal)?).map_err(internal)?
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
    [
        ("memory_open_session", "Open a memory session"),
        ("memory_capture_episode", "Capture one action/context/outcome episode"),
        ("memory_retrieve", "Retrieve structured memories for a query"),
        ("memory_consolidate", "Run consolidation and schema generation"),
        ("memory_get_working_context", "Read the active working context"),
        ("memory_update_working_context", "Open/update a working context"),
        ("memory_get_config", "Read runtime behavior config"),
        ("memory_update_config", "Update runtime behavior config"),
    ]
    .into_iter()
    .map(|(name, description)| json!({ "name": name, "description": description, "inputSchema": { "type": "object" } }))
    .collect()
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
        other => Err((-32602, format!("unknown session mode: {other}"))),
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
