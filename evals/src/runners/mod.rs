use anyhow::Result;
use std::path::PathBuf;

pub mod invoice;
pub mod ocr_quality;
pub mod threads;

pub async fn run_suite(suite: &str, dataset: Option<PathBuf>, model: &str) -> Result<()> {
    match suite {
        "invoice" => invoice::run(dataset, model).await,
        "ocr" | "ocr_quality" => ocr_quality::run(dataset, model).await,
        "threads" => threads::run(dataset, model).await,
        "all" => {
            invoice::run(None, model).await.ok();
            ocr_quality::run(None, model).await.ok();
            threads::run(None, model).await.ok();
            Ok(())
        }
        other => anyhow::bail!("unknown suite: {other}. Run `evals list` to see available suites."),
    }
}
