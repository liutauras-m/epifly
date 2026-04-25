use anyhow::Result;
use std::path::PathBuf;

pub mod invoice;

pub async fn run_suite(suite: &str, dataset: Option<PathBuf>, model: &str) -> Result<()> {
    match suite {
        "invoice" => invoice::run(dataset, model).await,
        other => anyhow::bail!("unknown suite: {other}. Run `evals list` to see available suites."),
    }
}
