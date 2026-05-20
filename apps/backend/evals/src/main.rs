use anyhow::Result;
use clap::{Parser, Subcommand};
use runners::generic::ScorerKind;

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
        /// Suite name (e.g. invoice, ocr, all)
        #[arg(short, long)]
        suite: String,
        /// Dataset file (JSONL) — overrides the suite's default dataset
        #[arg(short, long)]
        dataset: Option<std::path::PathBuf>,
        /// Claude model to use
        #[arg(long, default_value = "claude-opus-4-7")]
        model: String,
        /// Scoring strategy: exact | field-diff | llm-judge | default
        #[arg(long, default_value = "default")]
        scorer: String,
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
        Commands::Run { suite, dataset, model, scorer } => {
            let scorer_kind: ScorerKind = scorer.parse()?;
            runners::run_suite_with_scorer(&suite, dataset, &model, scorer_kind).await?;
        }
        Commands::List => {
            println!("Available suites:");
            println!("  smoke       - Smoke test (evals/suites/smoke.jsonl)");
            println!("  invoice     - Invoice extraction (evals/suites/invoice.jsonl)");
            println!("  ocr         - OCR text extraction (evals/suites/ocr.jsonl)");
            println!("  all         - Run all suites sequentially");
            println!();
            println!("Scorer options (--scorer):");
            println!("  default     - Each suite's built-in scorer");
            println!("  exact       - Exact JSON equality");
            println!("  field-diff  - Field-by-field diff (auto-discovers expected keys)");
            println!("  llm-judge   - LLM-graded evaluation (requires GATEWAY_URL + EVAL_TENANT_ID)");
        }
    }
    Ok(())
}
