use clap::Parser;

#[derive(Parser, Debug)]
struct Cli {
    #[arg(long, default_value = "engram-cli placeholder")]
    message: String,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    println!("{}", cli.message);
}
