use std::net::SocketAddr;

use engram_qwen::{DashScopeClient, DashScopeConfig, LlmClient, LlmConfig};
use engram_server::{AppState, build_app, mcp, routes::{LogBuffer, LogCaptureLayer}};
use tokio::net::TcpListener;
use tracing_subscriber::{EnvFilter, Layer, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file if present so LLM_API_KEY and LLM_ENDPOINT are available.
    // This is a no-op if the file is missing or cannot be read.
    let _ = dotenvy::dotenv();

    let log_buffer = LogBuffer::new();
    init_tracing(log_buffer.clone());

    if std::env::args().nth(1).as_deref() == Some("mcp-stdio") {
        let state = app_state_from_env(log_buffer).await?;
        mcp::run_stdio(state).await?;
        return Ok(());
    }

    let addr = std::env::var("ENGRAM_SERVER_ADDR")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or_else(|| SocketAddr::from(([127, 0, 0, 1], 3000)));

    let listener = TcpListener::bind(addr).await?;
    let app = build_app(app_state_from_env(log_buffer).await?);

    println!("engram-server listening on http://{addr}");
    axum::serve(listener, app).await?;

    Ok(())
}

fn init_tracing(log_buffer: LogBuffer) {
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_filter(EnvFilter::from_default_env());
    
    let capture_layer = LogCaptureLayer::new(log_buffer);
    
    let _ = tracing_subscriber::registry()
        .with(fmt_layer)
        .with(capture_layer)
        .try_init();
}

async fn app_state_from_env(log_buffer: LogBuffer) -> anyhow::Result<AppState> {
    let mut state = AppState::default();
    state.log_buffer = log_buffer;
    let require_llm = std::env::var("ENGRAM_REQUIRE_LLM")
        .ok()
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    if let Ok(endpoint) = std::env::var("LLM_ENDPOINT") {
        let config = LlmConfig::from_env()?;
        let client = LlmClient::new(config);
        state.system = state.system.with_llm(client);
        println!("[engram-server] LLM client connected: {}", endpoint);
    } else if let Ok(api_key) = std::env::var("ENGRAM_DASHSCOPE_API_KEY") {
        let client = DashScopeClient::new(DashScopeConfig::new(api_key)?);
        state.system = state.system.with_qwen(client);
        println!("[engram-server] Qwen client connected (DashScope).");
    } else {
        let msg = "[engram-server] LLM client NOT connected: LLM_ENDPOINT or ENGRAM_DASHSCOPE_API_KEY is missing. Chat will use local fallback replies.";
        if require_llm {
            anyhow::bail!(
                "{} Set LLM_ENDPOINT/LLM_API_KEY or ENGRAM_DASHSCOPE_API_KEY, or unset ENGRAM_REQUIRE_LLM.",
                msg
            );
        }
        println!("{}", msg);
    }

    // Retry initialization with exponential backoff so the server survives
    // transient Neo4j unavailability during container startup.
    let mut backoff = std::time::Duration::from_millis(500);
    let max_backoff = std::time::Duration::from_secs(30);
    loop {
        match state.system.initialize().await {
            Ok(()) => break,
            Err(err) => {
                tracing::warn!("Initialization failed (will retry): {err}");
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(max_backoff);
            }
        }
    }
    Ok(state)
}
