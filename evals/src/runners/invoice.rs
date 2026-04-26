use agent_core::chains::invoice::InvoicePipeline;
use anyhow::Result;
use std::path::PathBuf;
use tracing::info;

use crate::report::print_report;
use crate::scorers::{InvoiceScorer, ScorerResult};

#[derive(serde::Deserialize)]
struct EvalSample {
    image_path: String,
    expected: serde_json::Value,
}

pub async fn run(dataset: Option<PathBuf>, model: &str) -> Result<()> {
    let dataset_path = dataset.unwrap_or_else(|| PathBuf::from("evals/datasets/invoice.jsonl"));

    anyhow::ensure!(
        dataset_path.exists(),
        "Dataset not found: {:?}\nCreate it with sample records.",
        dataset_path
    );

    let content = std::fs::read_to_string(&dataset_path)?;
    let samples: Vec<EvalSample> = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).map_err(anyhow::Error::from))
        .collect::<Result<_>>()?;

    println!(
        "Running invoice eval suite — {} samples, model: {}",
        samples.len(),
        model
    );

    let pipeline = InvoicePipeline::with_model(model);
    let scorer = InvoiceScorer::new();
    let mut results: Vec<ScorerResult> = Vec::new();

    for (i, sample) in samples.iter().enumerate() {
        info!(i, path = %sample.image_path, "evaluating sample");
        print!("  [{}/{}] {} ... ", i + 1, samples.len(), sample.image_path);

        let path = PathBuf::from(&sample.image_path);
        match pipeline.extract_from_image_path(&path).await {
            Ok(extracted) => {
                let result = scorer.score(&extracted, &sample.expected);
                let status = if result.passed { "✅" } else { "❌" };
                println!("{} score={:.2}", status, result.score);
                results.push(result);
            }
            Err(e) => {
                println!("❌ ERROR: {e}");
                results.push(ScorerResult {
                    score: 0.0,
                    passed: false,
                    details: format!("error: {e}"),
                });
            }
        }
    }

    print_report(&results);
    Ok(())
}
