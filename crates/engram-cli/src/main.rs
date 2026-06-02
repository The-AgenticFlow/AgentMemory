use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use engram_core::SessionMode;
use reqwest::Url;
use serde::Serialize;

#[derive(Parser, Debug)]
#[command(name = "engram-cli")]
#[command(about = "Utility client for the Engram server")]
struct Cli {
    #[arg(long, default_value = "http://127.0.0.1:3000")]
    server: String,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Prints server health.
    Health,
    /// Opens a new session and prints the response.
    OpenSession {
        #[arg(long, default_value = "remember research preferences")]
        expectation: String,
        #[arg(long, default_value = "research assistant task")]
        task_context: String,
        #[arg(long, default_value = "exploration")]
        mode: String,
    },
    /// Sends one chat message to an existing session.
    Chat {
        #[arg(long)]
        session: String,
        #[arg(long)]
        message: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let server = Url::parse(&cli.server).context("invalid --server URL")?;
    let client = reqwest::Client::new();

    match cli.command {
        Command::Health => {
            let body = client
                .get(server.join("health")?)
                .send()
                .await?
                .error_for_status()?
                .text()
                .await?;
            println!("{}", body);
        }
        Command::OpenSession {
            expectation,
            task_context,
            mode,
        } => {
            let mode = parse_session_mode(&mode)?;
            let response = client
                .post(server.join("sessions")?)
                .json(&OpenSessionRequest {
                    user_id: None,
                    expectation,
                    mode,
                    task_context,
                })
                .send()
                .await?
                .error_for_status()?
                .text()
                .await?;
            println!("{}", response);
        }
        Command::Chat { session, message } => {
            let response = client
                .post(server.join(&format!("sessions/{session}/chat"))?)
                .json(&ChatRequest { message })
                .send()
                .await?
                .error_for_status()?
                .text()
                .await?;
            println!("{}", response);
        }
    }

    Ok(())
}

fn parse_session_mode(value: &str) -> Result<SessionMode> {
    match value.to_ascii_lowercase().as_str() {
        "exploration" => Ok(SessionMode::Exploration),
        "routine" => Ok(SessionMode::Routine),
        "critical" => Ok(SessionMode::Critical),
        other => anyhow::bail!("unknown session mode: {other}"),
    }
}

#[derive(Debug, Serialize)]
struct OpenSessionRequest {
    user_id: Option<uuid::Uuid>,
    expectation: String,
    mode: SessionMode,
    task_context: String,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    message: String,
}
