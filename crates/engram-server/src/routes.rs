use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use engram_core::{Session, SessionMode, WorkingContext};
use engram_qwen::chat::{ChatMessage, ChatRequest};
use engram_runtime::{IngestionOutcome, MemorySystem, RetrievalOutcome, SessionHandle};
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
        .route("/sessions", post(open_session))
        .route("/sessions/{id}", put(update_session).delete(close_session))
        .route(
            "/sessions/{id}/working-context",
            post(open_working_context).delete(close_working_context),
        )
        .route("/sessions/{id}/episodes", post(process_episode))
        .route("/sessions/{id}/retrieve", post(retrieve))
        .route("/sessions/{id}/chat", post(chat))
        .route("/sessions/{id}/ws", get(chat_ws))
        .route("/consolidate", post(consolidate))
        .with_state(state)
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub sessions: usize,
}

#[derive(Debug, Deserialize)]
pub struct SessionRequest {
    pub user_id: Option<Uuid>,
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
pub struct ChatRequestBody {
    pub message: String,
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
    pub reply: String,
    pub ingestion: IngestionOutcome,
}

#[derive(Debug, Serialize)]
pub struct ConsolidationResponse {
    pub created_schemas: usize,
}

pub async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    let sessions = state.sessions.read().await.len();
    Json(HealthResponse {
        status: "ok",
        sessions,
    })
}

pub async fn open_session(
    State(state): State<AppState>,
    Json(request): Json<SessionRequest>,
) -> ApiResult<Json<SessionResponse>> {
    let handle = state
        .system
        .open_session(
            request.user_id,
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
    let response = handle_chat_message(&state, session_id, request.message).await?;
    Ok(Json(response))
}

pub async fn chat_ws(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    ws: WebSocketUpgrade,
) -> ApiResult<impl axum::response::IntoResponse> {
    if !state.sessions.read().await.contains_key(&session_id) {
        return Err((StatusCode::NOT_FOUND, format!("session {session_id} not found")));
    }

    Ok(ws.on_upgrade(move |socket| async move {
        handle_chat_socket(state, session_id, socket).await;
    }))
}

pub async fn consolidate(
    State(state): State<AppState>,
) -> ApiResult<Json<ConsolidationResponse>> {
    let created = state.system.consolidate().await.map_err(internal_error)?;
    Ok(Json(ConsolidationResponse {
        created_schemas: created.len(),
    }))
}

async fn session_view(state: &AppState, session_id: Uuid) -> ApiResult<SessionView> {
    let sessions = state.sessions.read().await;
    sessions
        .get(&session_id)
        .map(|handle| SessionView {
            session: handle.session.clone(),
            working_context: handle.working_context.clone(),
        })
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("session {session_id} not found")))
}

async fn generate_reply(
    state: &AppState,
    handle: &SessionHandle,
    message: &str,
    retrieval: &RetrievalOutcome,
) -> String {
    if let Some(qwen) = &state.system.qwen {
        let prompt = build_prompt(handle, message, retrieval);
        if let Ok(response) = qwen
            .chat(&ChatRequest::new(
                "qwen3.6-plus",
                vec![
                    ChatMessage {
                        role: "system".to_string(),
                        content: "You are EngramAgent. Answer briefly, use retrieved memory when helpful, and mention gaps when memory is incomplete.".to_string(),
                    },
                    ChatMessage {
                        role: "user".to_string(),
                        content: prompt,
                    },
                ],
            ))
            .await
        {
            if let Some(choice) = response.choices.first() {
                return choice.message.content.clone();
            }
        }
    }

    fallback_reply(handle, message, retrieval)
}

async fn handle_chat_message(
    state: &AppState,
    session_id: Uuid,
    message: String,
) -> ApiResult<ChatResponse> {
    let mut handle = take_handle(state, session_id).await?;
    let result = handle_chat_session(state, session_id, &mut handle, message).await;
    put_handle(state, &handle).await;
    result
}

async fn handle_chat_session(
    state: &AppState,
    session_id: Uuid,
    handle: &mut SessionHandle,
    message: String,
) -> ApiResult<ChatResponse> {
    let retrieval = state
        .system
        .retrieve(&handle, message.clone())
        .await
        .map_err(internal_error)?;
    let reply = generate_reply(state, &handle, &message, &retrieval).await;
    let task_context = handle.session.task_context.clone();
    let ingestion = state
        .system
        .process_episode(
            handle,
            format!("answered user message: {}", message),
            task_context,
            reply.clone(),
        )
        .await
        .map_err(internal_error)?;
    let session = session_view(state, session_id).await?;

    Ok(ChatResponse {
        session,
        retrieval,
        reply,
        ingestion,
    })
}

async fn handle_chat_socket(state: AppState, session_id: Uuid, mut socket: WebSocket) {
    while let Some(Ok(message)) = socket.next().await {
        let Message::Text(text) = message else {
            continue;
        };

        match handle_chat_message(&state, session_id, text.to_string()).await {
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

fn build_prompt(handle: &SessionHandle, message: &str, retrieval: &RetrievalOutcome) -> String {
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
        "Task context: {}\nCurrent expectation: {}\nUser message: {}\n\nFacts:\n- {}\n\nInferences:\n- {}\n\nGaps:\n- {}\n\nReply in a concise, helpful way.",
        handle.session.task_context,
        handle.session.current_expectation,
        message,
        facts,
        inferences,
        gaps
    )
}

fn fallback_reply(
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

async fn take_handle(state: &AppState, session_id: Uuid) -> ApiResult<SessionHandle> {
    state
        .sessions
        .write()
        .await
        .remove(&session_id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("session {session_id} not found")))
}

async fn put_handle(state: &AppState, handle: &SessionHandle) {
    state
        .sessions
        .write()
        .await
        .insert(handle.session.id, handle.clone());
}

fn internal_error(error: anyhow::Error) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}
