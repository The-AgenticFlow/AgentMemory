use std::net::SocketAddr;

use engram_qwen::{DashScopeClient, DashScopeConfig};
use engram_server::{build_app, AppState};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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
    if let Ok(api_key) = std::env::var("ENGRAM_DASHSCOPE_API_KEY") {
        let client = DashScopeClient::new(DashScopeConfig::new(api_key)?);
        state.system = state.system.with_qwen(client);
    }
    Ok(state)
}
