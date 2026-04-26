use anyhow::Result;
use clap::{Parser, Subcommand};

mod config;
mod report;
mod runners;
mod scorers;

#[derive(Parser)]
#[command(name = "evals", about = "ConusAI evaluation framework")]
struct Cli {
    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run an evaluation suite
    Run {
        /// Suite name (e.g. invoice)
        #[arg(short, long)]
        suite: String,
        /// Dataset file (JSONL)
        #[arg(short, long)]
        dataset: Option<std::path::PathBuf>,
        /// Claude model to use
        #[arg(long, default_value = "claude-opus-4-7")]
        model: String,
    },
    /// List available suites
    List,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();
    match cli.cmd {
        Commands::Run {
            suite,
            dataset,
            model,
        } => {
            runners::run_suite(&suite, dataset, &model).await?;
        }
        Commands::List => {
            println!("Available suites:");
            println!("  invoice     - Invoice extraction accuracy evaluation");
            println!("  ocr         - OCR text extraction quality evaluation");
            println!("  threads     - Multi-turn thread memory recall evaluation");
            println!("  all         - Run all suites sequentially");
        }
    }
    Ok(())
}
