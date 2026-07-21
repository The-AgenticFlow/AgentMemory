use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use engram_core::{
    BankType, DispositionConfig, Episode, MemoryBank, RetrievalState, Session, SessionMode,
    WorkingContext,
};
use engram_qwen::chat::{ChatMessage, ChatRequest};
use engram_runtime::{
    ConstructiveKnowledge, IngestionOutcome, MemorySystem, RetrievalOutcome, RuntimeConfig,
    SessionHandle,
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

type ApiResult<T> = Result<T, (StatusCode, String)>;

#[derive(Clone)]
pub struct AppState {
    pub system: MemorySystem,
    pub sessions: Arc<RwLock<HashMap<Uuid, SessionHandle>>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        Self {
            system: MemorySystem::new(),
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/mcp", post(crate::mcp::mcp_http))
        .route("/control/overview", get(control_overview))
        .route("/control/graph", get(control_graph))
        .route(
            "/control/config",
            get(get_control_config).put(update_control_config),
        )
        .route("/control/config/reset", post(reset_control_config))
        .route("/control/simulate/thalamus", post(simulate_thalamus))
        .route("/memory/episodes", get(list_episodes))
        .route("/memory/patterns", get(list_patterns))
        .route("/memory/engrams", get(list_engrams))
        .route("/memory/schemas", get(list_schemas))
        .route("/sessions", get(list_sessions).post(open_session))
        .route("/sessions/{id}", put(update_session).delete(close_session))
        .route("/sessions/{id}/delete", delete(delete_session))
        .route("/sessions/{id}/view", get(get_session_view))
        .route(
            "/sessions/{id}/working-context",
            post(open_working_context).delete(close_working_context),
        )
        .route("/sessions/{id}/episodes", post(process_episode))
        .route("/sessions/{id}/retrieve", post(retrieve))
        .route("/sessions/{id}/chat", post(chat))
        .route("/sessions/{id}/ws", get(chat_ws))
        .route("/consolidate", post(consolidate))
        .route("/banks", get(list_banks).post(create_bank))
        .route(
            "/banks/{id}",
            get(get_bank).put(update_bank).delete(delete_bank),
        )
        .route("/banks/{id}/sessions", get(list_bank_sessions))
        .route("/banks/{id}/engrams", get(list_bank_engrams))
        .route("/banks/{id}/schemas", get(list_bank_schemas))
        .route("/working-memory", get(list_working_memory))
        .route(
            "/sessions/{id}/working-memory",
            post(add_working_memory).get(list_session_working_memory),
        )
        .route(
            "/sessions/{id}/working-memory/{entry_id}",
            delete(delete_working_memory),
        )
        .route("/sessions/{id}/retain", post(process_episode))
        .route("/sessions/{id}/recall", post(retrieve))
        .route("/reflect", post(consolidate))
        .with_state(state)
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub sessions: usize,
    pub qwen_connected: bool,
    pub llm_connected: bool,
}

#[derive(Debug, Deserialize)]
pub struct SessionRequest {
    pub user_id: Option<Uuid>,
    pub bank_id: Option<Uuid>,
    pub expectation: String,
    pub mode: SessionMode,
    pub task_context: String,
}

#[derive(Debug, Deserialize)]
pub struct WorkingContextRequest {
    pub task_id: String,
}

#[derive(Debug, Deserialize)]
pub struct EpisodeRequest {
    pub action: String,
    pub context: String,
    pub outcome: String,
}

#[derive(Debug, Deserialize)]
pub struct RetrievalRequest {
    pub query: String,
}

#[derive(Debug, Deserialize)]
pub struct ThalamusSimRequest {
    pub session_id: Option<Uuid>,
    pub action: String,
    pub context: String,
    pub outcome: String,
    pub expectation: Option<String>,
    pub mode: Option<SessionMode>,
    pub task_context: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BankQuery {
    pub bank_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct ChatRequestBody {
    pub message: String,
    #[serde(default)]
    pub retrieval_enabled: bool,
    #[serde(default)]
    pub debug: bool,
}

/// Debug snapshot exposed when the client requests verbose memory internals.
#[derive(Debug, Serialize)]
pub struct DebugSnapshot {
    pub episode: EpisodeDebug,
    pub ingestion: IngestionDebug,
    pub retrieval: Option<RetrievalDebug>,
}

#[derive(Debug, Serialize)]
pub struct EpisodeDebug {
    pub action: String,
    pub context: String,
    pub outcome: String,
}

#[derive(Debug, Serialize)]
pub struct IngestionDebug {
    pub accepted: bool,
    pub score: f32,
    pub pattern_hash: Option<String>,
    pub engram_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RetrievalDebug {
    pub mode: String,
    pub candidate_count: usize,
    pub schema_matched: bool,
    pub facts_count: usize,
    pub inferences_count: usize,
    pub gaps_count: usize,
}

#[derive(Debug, Serialize)]
pub struct SessionView {
    pub session: Session,
    pub working_context: Option<WorkingContext>,
}

#[derive(Debug, Serialize)]
pub struct SessionResponse {
    pub session: SessionView,
}

#[derive(Debug, Serialize)]
pub struct SessionSummary {
    pub session: SessionView,
    pub title: String,
    pub subtitle: String,
}

#[derive(Debug, Serialize)]
pub struct EpisodeResponse {
    pub session: SessionView,
    pub ingestion: IngestionOutcome,
}

#[derive(Debug, Serialize)]
pub struct RetrievalResponse {
    pub session: SessionView,
    pub retrieval: RetrievalOutcome,
}

#[derive(Debug, Serialize)]
pub struct ChatResponse {
    pub session: SessionView,
    pub retrieval: RetrievalOutcome,
    pub retrieval_enabled: bool,
    pub reply: String,
    pub ingestion: IngestionOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debug: Option<DebugSnapshot>,
}

#[derive(Debug, Serialize)]
pub struct ConsolidationResponse {
    pub created_schemas: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debug: Option<Vec<SchemaDebug>>,
}

#[derive(Debug, Serialize)]
pub struct SchemaDebug {
    pub id: String,
    pub tags: Vec<String>,
    pub prediction_fields: Vec<String>,
    pub source_engram_ids: Vec<String>,
}

pub async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    let sessions = state.sessions.read().await.len();
    Json(HealthResponse {
        status: "ok",
        sessions,
        qwen_connected: state.system.qwen.is_some(),
        llm_connected: state.system.llm.is_some(),
    })
}

pub async fn control_overview(
    State(state): State<AppState>,
    Query(query): Query<BankQuery>,
) -> ApiResult<Json<engram_runtime::RuntimeOverview>> {
    Ok(Json(
        state
            .system
            .overview(query.bank_id)
            .await
            .map_err(internal_error)?,
    ))
}

pub async fn control_graph(
    State(state): State<AppState>,
    Query(query): Query<BankQuery>,
) -> ApiResult<Json<engram_runtime::ControlGraph>> {
    Ok(Json(
        state
            .system
            .control_graph(query.bank_id)
            .await
            .map_err(internal_error)?,
    ))
}

pub async fn get_control_config(State(state): State<AppState>) -> Json<RuntimeConfig> {
    Json(state.system.runtime_config())
}

pub async fn update_control_config(
    State(state): State<AppState>,
    Json(config): Json<RuntimeConfig>,
) -> ApiResult<Json<RuntimeConfig>> {
    let config = state
        .system
        .update_config("dashboard", config)
        .await
        .map_err(internal_error)?;
    Ok(Json(config))
}

pub async fn reset_control_config(State(state): State<AppState>) -> ApiResult<Json<RuntimeConfig>> {
    let config = state
        .system
        .reset_config("dashboard")
        .await
        .map_err(internal_error)?;
    Ok(Json(config))
}

pub async fn simulate_thalamus(
    State(state): State<AppState>,
    Json(request): Json<ThalamusSimRequest>,
) -> ApiResult<Json<engram_runtime::ThalamusSimulation>> {
    let session = if let Some(session_id) = request.session_id {
        load_handle(&state, session_id).await?.session
    } else {
        Session::new(
            None,
            None,
            request
                .expectation
                .unwrap_or_else(|| "preview expectation".to_string()),
            request.mode.unwrap_or(SessionMode::Exploration),
            request
                .task_context
                .unwrap_or_else(|| request.context.clone()),
        )
    };
    let simulation = state
        .system
        .simulate_thalamus(&session, request.action, request.context, request.outcome)
        .await
        .map_err(internal_error)?;
    Ok(Json(simulation))
}

pub async fn list_episodes(
    State(state): State<AppState>,
    Query(query): Query<BankQuery>,
) -> ApiResult<Json<Vec<Episode>>> {
    let episodes = if let Some(bank_id) = query.bank_id {
        state
            .system
            .postgres
            .list_episodes_by_bank(bank_id)
            .await
            .map_err(internal_error)?
    } else {
        state
            .system
            .postgres
            .list_episodes()
            .await
            .map_err(internal_error)?
    };
    Ok(Json(episodes))
}

pub async fn list_patterns(
    State(state): State<AppState>,
    Query(query): Query<BankQuery>,
) -> ApiResult<Json<Vec<engram_core::PatternEntry>>> {
    let patterns = if let Some(bank_id) = query.bank_id {
        state
            .system
            .qdrant
            .list_patterns_by_bank(bank_id)
            .await
            .map_err(internal_error)?
    } else {
        state
            .system
            .qdrant
            .list_patterns()
            .await
            .map_err(internal_error)?
    };
    Ok(Json(patterns))
}

pub async fn list_engrams(
    State(state): State<AppState>,
    Query(query): Query<BankQuery>,
) -> ApiResult<Json<Vec<engram_core::EngramEntry>>> {
    let engrams = if let Some(bank_id) = query.bank_id {
        state
            .system
            .qdrant
            .list_engrams_by_bank(bank_id)
            .await
            .map_err(internal_error)?
    } else {
        state
            .system
            .qdrant
            .list_engrams()
            .await
            .map_err(internal_error)?
    };
    Ok(Json(engrams))
}

pub async fn list_schemas(
    State(state): State<AppState>,
    Query(query): Query<BankQuery>,
) -> ApiResult<Json<Vec<engram_core::MetaEngram>>> {
    let schemas = if let Some(bank_id) = query.bank_id {
        state
            .system
            .postgres
            .list_schemas_by_bank(bank_id)
            .await
            .map_err(internal_error)?
    } else {
        state
            .system
            .postgres
            .list_schemas()
            .await
            .map_err(internal_error)?
    };
    Ok(Json(schemas))
}

pub async fn list_sessions(
    State(state): State<AppState>,
    Query(query): Query<BankQuery>,
) -> ApiResult<Json<Vec<SessionSummary>>> {
    let sessions = if let Some(bank_id) = query.bank_id {
        state
            .system
            .postgres
            .list_sessions_by_bank(bank_id)
            .await
            .map_err(internal_error)?
    } else {
        state
            .system
            .postgres
            .list_sessions()
            .await
            .map_err(internal_error)?
    };
    let mut summaries = Vec::with_capacity(sessions.len());

    for session in sessions {
        let working_context = state
            .system
            .postgres
            .get_working_context(session.id)
            .await
            .ok()
            .flatten();
        summaries.push(SessionSummary {
            title: session.task_context.clone(),
            subtitle: session.current_expectation.clone(),
            session: SessionView {
                session,
                working_context,
            },
        });
    }

    Ok(Json(summaries))
}

pub async fn open_session(
    State(state): State<AppState>,
    Json(request): Json<SessionRequest>,
) -> ApiResult<Json<SessionResponse>> {
    let handle = state
        .system
        .open_session(
            request.user_id,
            request.bank_id,
            request.expectation,
            request.mode,
            request.task_context,
        )
        .await
        .map_err(internal_error)?;

    let session_id = handle.session.id;
    state.sessions.write().await.insert(session_id, handle);
    let handle = session_view(&state, session_id).await?;

    Ok(Json(SessionResponse { session: handle }))
}

pub async fn update_session(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(request): Json<SessionRequest>,
) -> ApiResult<Json<SessionResponse>> {
    let mut handle = take_handle(&state, session_id).await?;
    state
        .system
        .update_session(
            &mut handle,
            request.expectation,
            request.mode,
            request.task_context,
        )
        .await
        .map_err(internal_error)?;
    put_handle(&state, &handle).await;
    let session = session_view(&state, session_id).await?;

    Ok(Json(SessionResponse { session }))
}

pub async fn close_session(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> ApiResult<Json<SessionResponse>> {
    let mut handle = take_handle(&state, session_id).await?;
    state
        .system
        .close_session(&mut handle)
        .await
        .map_err(internal_error)?;
    put_handle(&state, &handle).await;
    let session = session_view(&state, session_id).await?;

    Ok(Json(SessionResponse { session }))
}

pub async fn delete_session(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> ApiResult<Json<()>> {
    state.sessions.write().await.remove(&session_id);
    state
        .system
        .postgres
        .delete_session(session_id)
        .await
        .map_err(internal_error)?;

    Ok(Json(()))
}

pub async fn get_session_view(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> ApiResult<Json<SessionResponse>> {
    let handle = load_handle(&state, session_id).await?;
    Ok(Json(SessionResponse {
        session: session_view_from_handle(&handle),
    }))
}

pub async fn open_working_context(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(request): Json<WorkingContextRequest>,
) -> ApiResult<Json<SessionResponse>> {
    let mut handle = take_handle(&state, session_id).await?;
    state
        .system
        .open_working_context(&mut handle, request.task_id)
        .await
        .map_err(internal_error)?;
    put_handle(&state, &handle).await;
    let session = session_view(&state, session_id).await?;

    Ok(Json(SessionResponse { session }))
}

pub async fn close_working_context(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> ApiResult<Json<SessionResponse>> {
    let mut handle = take_handle(&state, session_id).await?;
    if let Some(mut context) = handle.working_context.take() {
        context.close();
        state
            .system
            .postgres
            .save_working_context(&context)
            .await
            .map_err(internal_error)?;
    }
    put_handle(&state, &handle).await;
    let session = session_view(&state, session_id).await?;

    Ok(Json(SessionResponse { session }))
}

pub async fn process_episode(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(request): Json<EpisodeRequest>,
) -> ApiResult<Json<EpisodeResponse>> {
    let mut handle = take_handle(&state, session_id).await?;
    let ingestion = state
        .system
        .process_episode(
            &mut handle,
            request.action,
            request.context,
            request.outcome,
        )
        .await
        .map_err(internal_error)?;
    put_handle(&state, &handle).await;
    let session = session_view(&state, session_id).await?;

    Ok(Json(EpisodeResponse { session, ingestion }))
}

pub async fn retrieve(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(request): Json<RetrievalRequest>,
) -> ApiResult<Json<RetrievalResponse>> {
    let handle = take_handle(&state, session_id).await?;
    let retrieval = state
        .system
        .retrieve(&handle, request.query)
        .await
        .map_err(internal_error)?;
    put_handle(&state, &handle).await;
    let session = session_view(&state, session_id).await?;

    Ok(Json(RetrievalResponse { session, retrieval }))
}

pub async fn chat(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(request): Json<ChatRequestBody>,
) -> ApiResult<Json<ChatResponse>> {
    let response = handle_chat_message(
        &state,
        session_id,
        request.message,
        request.retrieval_enabled,
        request.debug,
    )
    .await?;
    Ok(Json(response))
}

pub async fn chat_ws(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    ws: WebSocketUpgrade,
) -> ApiResult<impl axum::response::IntoResponse> {
    let _ = load_handle(&state, session_id).await?;

    Ok(ws.on_upgrade(move |socket| async move {
        handle_chat_socket(state, session_id, socket).await;
    }))
}

#[derive(Debug, Deserialize, Default)]
pub struct ConsolidateRequest {
    #[serde(default)]
    debug: bool,
}

pub async fn consolidate(
    State(state): State<AppState>,
    Json(request): Json<ConsolidateRequest>,
) -> ApiResult<Json<ConsolidationResponse>> {
    let created = state.system.consolidate().await.map_err(internal_error)?;
    let debug_info = if request.debug {
        Some(
            created
                .iter()
                .map(|schema| SchemaDebug {
                    id: schema.id.to_string(),
                    tags: schema.tags.clone(),
                    prediction_fields: schema.prediction_fields.clone(),
                    source_engram_ids: schema
                        .source_engram_ids
                        .iter()
                        .map(|id| id.to_string())
                        .collect(),
                })
                .collect(),
        )
    } else {
        None
    };
    Ok(Json(ConsolidationResponse {
        created_schemas: created.len(),
        debug: debug_info,
    }))
}

async fn session_view(state: &AppState, session_id: Uuid) -> ApiResult<SessionView> {
    let handle = load_handle(state, session_id).await?;
    Ok(session_view_from_handle(&handle))
}

fn get_llm_model() -> String {
    std::env::var("LLM_MODEL").unwrap_or_else(|_| "qwen-plus".to_string())
}

async fn generate_reply(
    state: &AppState,
    handle: &SessionHandle,
    message: &str,
    retrieval: Option<&RetrievalOutcome>,
) -> String {
    let prompt = match retrieval {
        Some(retrieval) => build_prompt_with_retrieval(handle, message, retrieval),
        None => build_prompt_plain(handle, message),
    };

    let system_prompt = "You are EngramAgent, a persistent-memory assistant. When retrieved memories are provided below, ground your answer ONLY in the numbered [1], [2]... facts shown. If the memories do not contain the answer, say so clearly and note the gap. Never invent facts or infer names, dates, or details that are not in the retrieved text.";

    let messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: system_prompt.to_string(),
        },
        ChatMessage {
            role: "user".to_string(),
            content: prompt,
        },
    ];

    if let Some(llm) = &state.system.llm {
        let model = get_llm_model();
        match llm.chat(&ChatRequest::new(model, messages.clone())).await {
            Ok(response) => {
                if let Some(choice) = response.choices.first() {
                    return choice.message.content.clone();
                }
            }
            Err(e) => {
                tracing::warn!("LLM chat call failed: {e}");
            }
        }
    } else if let Some(qwen) = &state.system.qwen {
        let model = get_llm_model();
        match qwen.chat(&ChatRequest::new(model, messages)).await {
            Ok(response) => {
                if let Some(choice) = response.choices.first() {
                    return choice.message.content.clone();
                }
            }
            Err(e) => {
                tracing::warn!("Qwen chat call failed: {e}");
            }
        }
    }

    match retrieval {
        Some(retrieval) => fallback_reply_with_retrieval(handle, message, retrieval),
        None => fallback_reply_plain(handle, message),
    }
}

async fn handle_chat_message(
    state: &AppState,
    session_id: Uuid,
    message: String,
    retrieval_enabled: bool,
    debug: bool,
) -> ApiResult<ChatResponse> {
    let mut handle = take_handle(state, session_id).await?;
    let result = handle_chat_session(state, &mut handle, message, retrieval_enabled, debug).await;
    put_handle(state, &handle).await;
    result
}

async fn handle_chat_session(
    state: &AppState,
    handle: &mut SessionHandle,
    message: String,
    retrieval_enabled: bool,
    debug: bool,
) -> ApiResult<ChatResponse> {
    let retrieval = if retrieval_enabled {
        Some(
            state
                .system
                .retrieve(handle, message.clone())
                .await
                .map_err(internal_error)?,
        )
    } else {
        None
    };
    let reply = generate_reply(state, handle, &message, retrieval.as_ref()).await;
    let task_context = handle.session.task_context.clone();
    let action = format!("answered user message: {}", message);
    let context = task_context.clone();
    let outcome = reply.clone();
    let ingestion = state
        .system
        .process_episode(handle, action.clone(), context.clone(), outcome.clone())
        .await
        .map_err(internal_error)?;

    let debug_info = if debug {
        Some(DebugSnapshot {
            episode: EpisodeDebug {
                action,
                context,
                outcome,
            },
            ingestion: IngestionDebug {
                accepted: ingestion.accepted,
                score: ingestion.score,
                pattern_hash: ingestion.pattern_hash.clone(),
                engram_id: ingestion.engram_id.map(|id| id.to_string()),
            },
            retrieval: retrieval.as_ref().map(|r| RetrievalDebug {
                mode: format!("{:?}", r.mode),
                candidate_count: r.candidates.len(),
                schema_matched: r.schema.is_some(),
                facts_count: r.knowledge.facts.len(),
                inferences_count: r.knowledge.inferences.len(),
                gaps_count: r.knowledge.gaps.len(),
            }),
        })
    } else {
        None
    };

    Ok(ChatResponse {
        session: session_view_from_handle(handle),
        retrieval: retrieval.unwrap_or_else(empty_retrieval_outcome),
        retrieval_enabled,
        reply,
        ingestion,
        debug: debug_info,
    })
}

async fn handle_chat_socket(state: AppState, session_id: Uuid, mut socket: WebSocket) {
    while let Some(Ok(message)) = socket.next().await {
        let Message::Text(text) = message else {
            continue;
        };

        let request = parse_ws_chat_request(&text);
        match handle_chat_message(
            &state,
            session_id,
            request.message,
            request.retrieval_enabled,
            request.debug,
        )
        .await
        {
            Ok(response) => {
                if socket
                    .send(Message::Text(
                        serde_json::to_string(&response)
                            .unwrap_or_else(|_| "{}".to_string())
                            .into(),
                    ))
                    .await
                    .is_err()
                {
                    break;
                }
            }
            Err((_, error)) => {
                let payload = serde_json::json!({ "error": error });
                if socket
                    .send(Message::Text(payload.to_string().into()))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        }
    }
}

fn build_prompt_with_retrieval(
    handle: &SessionHandle,
    message: &str,
    retrieval: &RetrievalOutcome,
) -> String {
    let facts = if retrieval.knowledge.facts.is_empty() {
        "none".to_string()
    } else {
        retrieval.knowledge.facts.join("\n- ")
    };
    let inferences = if retrieval.knowledge.inferences.is_empty() {
        "none".to_string()
    } else {
        retrieval.knowledge.inferences.join("\n- ")
    };
    let gaps = if retrieval.knowledge.gaps.is_empty() {
        "none".to_string()
    } else {
        retrieval.knowledge.gaps.join("\n- ")
    };

    format!(
        "Task context: {}\nCurrent expectation: {}\nUser message: {}\n\nRetrieved memory:\n\nFacts:\n- {}\n\nInferences:\n- {}\n\nGaps:\n- {}\n\nInstructions: Synthesize the retrieved memory above into your answer. Explicitly reference relevant facts, build on inferences, and mention any gaps that limit your response. Do not return the raw traces as a list—incorporate them naturally into your reply.",
        handle.session.task_context,
        handle.session.current_expectation,
        message,
        facts,
        inferences,
        gaps
    )
}

fn build_prompt_plain(handle: &SessionHandle, message: &str) -> String {
    format!(
        "Task context: {}\nCurrent expectation: {}\nUser message: {}\n\nReply naturally and helpfully without using retrieved memory unless it is directly relevant.",
        handle.session.task_context, handle.session.current_expectation, message
    )
}

fn fallback_reply_with_retrieval(
    handle: &SessionHandle,
    message: &str,
    retrieval: &RetrievalOutcome,
) -> String {
    let mut response = format!(
        "I heard: '{}'. In task '{}', ",
        message, handle.session.task_context
    );

    if retrieval.knowledge.facts.is_empty() {
        response.push_str("I do not have a strong memory match yet.");
    } else {
        response.push_str("I found these facts: ");
        response.push_str(&retrieval.knowledge.facts.join("; "));
        response.push('.');
    }

    if !retrieval.knowledge.inferences.is_empty() {
        response.push_str(" I can infer: ");
        response.push_str(&retrieval.knowledge.inferences.join("; "));
        response.push('.');
    }

    if !retrieval.knowledge.gaps.is_empty() {
        response.push_str(" Missing pieces: ");
        response.push_str(&retrieval.knowledge.gaps.join("; "));
        response.push('.');
    }

    response
}

fn fallback_reply_plain(handle: &SessionHandle, message: &str) -> String {
    format!(
        "I heard: '{}'. I'm working on '{}' and can help with the next step.",
        message, handle.session.task_context
    )
}

fn empty_retrieval_outcome() -> RetrievalOutcome {
    RetrievalOutcome {
        mode: RetrievalState::Default,
        candidates: Vec::new(),
        schema: None,
        knowledge: ConstructiveKnowledge {
            facts: Vec::new(),
            inferences: Vec::new(),
            gaps: Vec::new(),
        },
    }
}

#[derive(Debug, Deserialize)]
struct WsChatRequest {
    message: String,
    #[serde(default)]
    retrieval_enabled: bool,
    #[serde(default)]
    debug: bool,
}

fn parse_ws_chat_request(text: &str) -> WsChatRequest {
    serde_json::from_str(text).unwrap_or_else(|_| WsChatRequest {
        message: text.to_string(),
        retrieval_enabled: false,
        debug: false,
    })
}

async fn take_handle(state: &AppState, session_id: Uuid) -> ApiResult<SessionHandle> {
    if let Some(handle) = state.sessions.write().await.remove(&session_id) {
        return Ok(handle);
    }

    load_handle(state, session_id).await
}

async fn put_handle(state: &AppState, handle: &SessionHandle) {
    state
        .sessions
        .write()
        .await
        .insert(handle.session.id, handle.clone());
}

async fn load_handle(state: &AppState, session_id: Uuid) -> ApiResult<SessionHandle> {
    if let Some(handle) = state.sessions.read().await.get(&session_id).cloned() {
        return Ok(handle);
    }

    let session = state
        .system
        .postgres
        .get_session(session_id)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("session {session_id} not found"),
            )
        })?;
    let working_context = state
        .system
        .postgres
        .get_working_context(session_id)
        .await
        .map_err(internal_error)?;
    let handle = SessionHandle {
        session,
        working_context,
    };
    put_handle(state, &handle).await;
    Ok(handle)
}

// ── Bank Endpoints ──────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct BankRequest {
    pub name: String,
    pub owner_id: Option<Uuid>,
    pub bank_type: BankType,
    pub mission: Option<String>,
    pub directives: Vec<String>,
    pub disposition: Option<DispositionConfig>,
    pub parent_bank_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct BankResponse {
    pub bank: MemoryBank,
}

pub async fn list_banks(
    State(state): State<AppState>,
) -> ApiResult<Json<Vec<engram_core::BankSummary>>> {
    let banks = state
        .system
        .postgres
        .list_banks()
        .await
        .map_err(internal_error)?;
    let mut summaries = Vec::with_capacity(banks.len());
    for bank in banks {
        let memory_count = state
            .system
            .postgres
            .list_engrams()
            .await
            .map_err(internal_error)?
            .iter()
            .filter(|e| e.bank_id == Some(bank.id))
            .count();
        let schema_count = state
            .system
            .postgres
            .list_schemas()
            .await
            .map_err(internal_error)?
            .iter()
            .filter(|s| s.bank_id == Some(bank.id))
            .count();
        summaries.push(engram_core::BankSummary {
            id: bank.id,
            name: bank.name.clone(),
            bank_type: bank.bank_type,
            mission: bank.mission.clone(),
            directive_count: bank.directives.len(),
            memory_count,
            schema_count,
            owner_id: bank.owner_id,
            parent_bank_id: bank.parent_bank_id,
        });
    }
    Ok(Json(summaries))
}

pub async fn get_bank(
    State(state): State<AppState>,
    Path(bank_id): Path<Uuid>,
) -> ApiResult<Json<BankResponse>> {
    let bank = state
        .system
        .postgres
        .get_bank(bank_id)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("bank {bank_id} not found")))?;
    Ok(Json(BankResponse { bank }))
}

pub async fn create_bank(
    State(state): State<AppState>,
    Json(request): Json<BankRequest>,
) -> ApiResult<Json<BankResponse>> {
    let bank = MemoryBank::new(
        request.name,
        request.owner_id,
        request.bank_type,
        request.mission,
        request.directives,
        request.disposition.unwrap_or_default(),
        request.parent_bank_id,
    );
    state
        .system
        .postgres
        .save_bank(&bank)
        .await
        .map_err(internal_error)?;
    Ok(Json(BankResponse { bank }))
}

pub async fn update_bank(
    State(state): State<AppState>,
    Path(bank_id): Path<Uuid>,
    Json(request): Json<BankRequest>,
) -> ApiResult<Json<BankResponse>> {
    let mut bank = state
        .system
        .postgres
        .get_bank(bank_id)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("bank {bank_id} not found")))?;
    bank.update(
        request.mission,
        request.directives,
        request.disposition.unwrap_or_default(),
    );
    state
        .system
        .postgres
        .save_bank(&bank)
        .await
        .map_err(internal_error)?;
    Ok(Json(BankResponse { bank }))
}

pub async fn delete_bank(
    State(state): State<AppState>,
    Path(bank_id): Path<Uuid>,
) -> ApiResult<Json<()>> {
    state
        .system
        .postgres
        .delete_bank(bank_id)
        .await
        .map_err(internal_error)?;
    Ok(Json(()))
}

pub async fn list_bank_sessions(
    State(state): State<AppState>,
    Path(bank_id): Path<Uuid>,
) -> ApiResult<Json<Vec<Session>>> {
    let sessions = state
        .system
        .postgres
        .list_sessions_by_bank(bank_id)
        .await
        .map_err(internal_error)?;
    Ok(Json(sessions))
}

pub async fn list_bank_engrams(
    State(state): State<AppState>,
    Path(bank_id): Path<Uuid>,
) -> ApiResult<Json<Vec<engram_core::EngramEntry>>> {
    let engrams = state
        .system
        .postgres
        .list_engrams_by_bank(bank_id)
        .await
        .map_err(internal_error)?;
    Ok(Json(engrams))
}

pub async fn list_bank_schemas(
    State(state): State<AppState>,
    Path(bank_id): Path<Uuid>,
) -> ApiResult<Json<Vec<engram_core::MetaEngram>>> {
    let schemas = state
        .system
        .postgres
        .list_schemas_by_bank(bank_id)
        .await
        .map_err(internal_error)?;
    Ok(Json(schemas))
}

// ── Working Memory Endpoints ────────────────────────────────────

pub async fn list_working_memory(
    State(state): State<AppState>,
    Query(query): Query<BankQuery>,
) -> ApiResult<Json<Vec<engram_core::WorkingMemoryEntry>>> {
    let entries = if let Some(bank_id) = query.bank_id {
        state
            .system
            .postgres
            .list_working_memory_by_bank(bank_id)
            .await
            .map_err(internal_error)?
    } else {
        state
            .system
            .postgres
            .list_working_memory()
            .await
            .map_err(internal_error)?
    };
    Ok(Json(entries))
}

pub async fn list_session_working_memory(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> ApiResult<Json<Vec<engram_core::WorkingMemoryEntry>>> {
    let entries = state
        .system
        .postgres
        .list_working_memory_by_session(session_id)
        .await
        .map_err(internal_error)?;
    Ok(Json(entries))
}

#[derive(Debug, Deserialize)]
pub struct WorkingMemoryRequest {
    pub content: String,
    pub tags: Vec<String>,
}

pub async fn add_working_memory(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(request): Json<WorkingMemoryRequest>,
) -> ApiResult<Json<engram_core::WorkingMemoryEntry>> {
    let handle = load_handle(&state, session_id).await?;
    let embedding = state.system.pseudo_embedding(&request.content);
    let entry = engram_core::WorkingMemoryEntry::new(
        session_id,
        handle.session.bank_id,
        request.content,
        embedding,
        request.tags,
    );
    state
        .system
        .postgres
        .save_working_memory(&entry)
        .await
        .map_err(internal_error)?;
    Ok(Json(entry))
}

pub async fn delete_working_memory(
    State(state): State<AppState>,
    Path((_session_id, entry_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<()>> {
    state
        .system
        .postgres
        .delete_working_memory(entry_id)
        .await
        .map_err(internal_error)?;
    Ok(Json(()))
}

fn session_view_from_handle(handle: &SessionHandle) -> SessionView {
    SessionView {
        session: handle.session.clone(),
        working_context: handle.working_context.clone(),
    }
}

fn internal_error(error: anyhow::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}
