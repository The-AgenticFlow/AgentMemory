use anyhow::Result;
use clap::{Parser, Subcommand};
use engram_core::SessionMode;
use engram_runtime::MemorySystem;

#[derive(Parser, Debug)]
#[command(name = "engram-cli")]
#[command(about = "Local demo and smoke-test CLI for the Engram memory runtime")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Runs a one-shot ingest and retrieve demo.
    Demo {
        #[arg(long, default_value = "remember research preferences")]
        expectation: String,
        #[arg(long, default_value = "research assistant task")]
        task_context: String,
        #[arg(long, default_value = "read paper on memory consolidation")]
        action: String,
        #[arg(long, default_value = "successfully stored the paper summary")]
        outcome: String,
        #[arg(long, default_value = "memory consolidation paper")]
        query: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Demo {
            expectation,
            task_context,
            action,
            outcome,
            query,
        } => demo(expectation, task_context, action, outcome, query).await?,
    }
    Ok(())
}

async fn demo(
    expectation: String,
    task_context: String,
    action: String,
    outcome: String,
    query: String,
) -> Result<()> {
    let system = MemorySystem::new();
    let mut handle = system
        .open_session(None, expectation, SessionMode::Exploration, task_context.clone())
        .await?;

    let ingestion = system
        .process_episode(&mut handle, action, task_context, outcome)
        .await?;
    let retrieval = system.retrieve(&handle, query).await?;
    let created_schemas = system.consolidate().await?;

    println!("{}", serde_json::to_string_pretty(&ingestion)?);
    println!("{}", serde_json::to_string_pretty(&retrieval)?);
    println!("created_schemas: {}", created_schemas.len());

    Ok(())
}
