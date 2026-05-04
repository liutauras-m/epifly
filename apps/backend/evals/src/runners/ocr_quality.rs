/// OCR quality eval: sends an image through the ocr-service capability and scores
/// the extracted text against known ground-truth snippets.
use anyhow::Result;
use serde_json::{Value, json};
use std::path::PathBuf;
use tracing::info;

use crate::report::print_report;
use crate::scorers::ScorerResult;

#[derive(serde::Deserialize)]
struct OcrSample {
    image_path: String,
    /// Substrings that must appear in the OCR output (case-insensitive).
    expected_snippets: Vec<String>,
}

pub async fn run(dataset: Option<PathBuf>, _model: &str) -> Result<()> {
    let dataset_path = dataset.unwrap_or_else(|| PathBuf::from("evals/datasets/ocr_quality.jsonl"));

    anyhow::ensure!(
        dataset_path.exists(),
        "Dataset not found: {:?}\nCreate it at evals/datasets/ocr_quality.jsonl",
        dataset_path
    );

    let gateway_url =
        std::env::var("GATEWAY_URL").unwrap_or_else(|_| "http://localhost:8080".into());
    let tenant_id = std::env::var("EVAL_TENANT_ID").unwrap_or_else(|_| "eval".into());

    let content = std::fs::read_to_string(&dataset_path)?;
    let samples: Vec<OcrSample> = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).map_err(anyhow::Error::from))
        .collect::<Result<_>>()?;

    println!(
        "Running OCR quality eval — {} samples, gateway: {}",
        samples.len(),
        gateway_url
    );

    let http = reqwest::Client::new();
    let mut results: Vec<ScorerResult> = Vec::new();

    for (i, sample) in samples.iter().enumerate() {
        info!(i, path = %sample.image_path, "evaluating OCR sample");
        print!("  [{}/{}] {} ... ", i + 1, samples.len(), sample.image_path);

        match run_ocr_sample(&http, &gateway_url, &tenant_id, sample).await {
            Ok((text, score)) => {
                let passed = score >= 0.8;
                let status = if passed { "✅" } else { "❌" };
                println!("{} score={:.2}", status, score);
                results.push(ScorerResult {
                    score,
                    passed,
                    details: text,
                });
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

async fn run_ocr_sample(
    http: &reqwest::Client,
    gateway_url: &str,
    tenant_id: &str,
    sample: &OcrSample,
) -> Result<(String, f64)> {
    let resp: Value = http
        .post(format!("{gateway_url}/v1/agent/completions"))
        .header("X-Tenant-ID", tenant_id)
        .json(&json!({
            "messages": [{
                "role": "user",
                "content": format!(
                    "Use the ocr-service to extract text from this image: {}",
                    sample.image_path
                )
            }]
        }))
        .send()
        .await?
        .json()
        .await?;

    let text = resp["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    let text_lower = text.to_lowercase();
    let hits = sample
        .expected_snippets
        .iter()
        .filter(|s| text_lower.contains(s.to_lowercase().as_str()))
        .count();

    let score = if sample.expected_snippets.is_empty() {
        1.0
    } else {
        hits as f64 / sample.expected_snippets.len() as f64
    };

    Ok((text, score))
}
