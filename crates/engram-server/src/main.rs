use std::net::SocketAddr;

use engram_qwen::{DashScopeClient, DashScopeConfig};
use engram_server::{build_app, AppState};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file if present so ENGRAM_DASHSCOPE_API_KEY is available.
    // This is a no-op if the file is missing or cannot be read.
    let _ = dotenvy::dotenv();

    init_tracing();

    let addr = std::env::var("ENGRAM_SERVER_ADDR")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or_else(|| SocketAddr::from(([127, 0, 0, 1], 3000)));

    let listener = TcpListener::bind(addr).await?;
    let app = build_app(app_state_from_env()?);

    println!("engram-server listening on http://{addr}");
    axum::serve(listener, app).await?;

    Ok(())
}

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();
}

fn app_state_from_env() -> anyhow::Result<AppState> {
    let mut state = AppState::default();
    let require_qwen = std::env::var("ENGRAM_REQUIRE_QWEN")
        .ok()
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    match std::env::var("ENGRAM_DASHSCOPE_API_KEY") {
        Ok(api_key) => {
            let client = DashScopeClient::new(DashScopeConfig::new(api_key)?);
            state.system = state.system.with_qwen(client);
            println!("[engram-server] Qwen client connected (DashScope).");
        }
        Err(_) => {
            let msg = "[engram-server] Qwen client NOT connected: ENGRAM_DASHSCOPE_API_KEY is missing. Chat will use local fallback replies.";
            if require_qwen {
                anyhow::bail!("{} Set it or unset ENGRAM_REQUIRE_QWEN.", msg);
            }
            println!("{}", msg);
        }
    }

    Ok(state)
}
