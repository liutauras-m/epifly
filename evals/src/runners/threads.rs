/// Thread multi-turn eval: creates a thread, runs N conversation turns via the gateway,
/// then scores whether the agent correctly recalled information from earlier turns.
use anyhow::Result;
use serde_json::{json, Value};
use tracing::info;

use crate::report::print_report;
use crate::scorers::ScorerResult;

#[derive(serde::Deserialize)]
struct ThreadEvalSample {
    /// Human turns in order; the agent replies to each one.
    turns: Vec<String>,
    /// A final question to test recall.
    recall_question: String,
    /// Keywords that must appear in the recall answer.
    expected_keywords: Vec<String>,
}

pub async fn run(dataset: Option<std::path::PathBuf>, _model: &str) -> Result<()> {
    let dataset_path = dataset
        .unwrap_or_else(|| std::path::PathBuf::from("evals/datasets/threads.jsonl"));

    anyhow::ensure!(
        dataset_path.exists(),
        "Dataset not found: {:?}\nCreate it at evals/datasets/threads.jsonl",
        dataset_path
    );

    let gateway_url = std::env::var("GATEWAY_URL")
        .unwrap_or_else(|_| "http://localhost:8080".into());
    let tenant_id = std::env::var("EVAL_TENANT_ID").unwrap_or_else(|_| "eval".into());

    let content = std::fs::read_to_string(&dataset_path)?;
    let samples: Vec<ThreadEvalSample> = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).map_err(anyhow::Error::from))
        .collect::<Result<_>>()?;

    println!(
        "Running thread recall eval — {} samples, gateway: {}",
        samples.len(),
        gateway_url
    );

    let http = reqwest::Client::new();
    let mut results: Vec<ScorerResult> = Vec::new();

    for (i, sample) in samples.iter().enumerate() {
        info!(i, turns = sample.turns.len(), "evaluating thread sample");
        print!("  [{}/{}] {} turns ... ", i + 1, samples.len(), sample.turns.len());

        match run_sample(&http, &gateway_url, &tenant_id, sample).await {
            Ok((recall_answer, score)) => {
                let passed = score >= 0.8;
                let status = if passed { "✅" } else { "❌" };
                println!("{} score={:.2} answer='{}'", status, score, &recall_answer[..recall_answer.len().min(60)]);
                results.push(ScorerResult {
                    score,
                    passed,
                    details: recall_answer,
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

async fn run_sample(
    http: &reqwest::Client,
    gateway_url: &str,
    tenant_id: &str,
    sample: &ThreadEvalSample,
) -> Result<(String, f64)> {
    // Create thread
    let thread_resp: Value = http
        .post(format!("{gateway_url}/v1/threads"))
        .header("X-Tenant-ID", tenant_id)
        .json(&json!({ "messages": [] }))
        .send()
        .await?
        .json()
        .await?;

    let thread_id = thread_resp["id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("create thread returned no id: {thread_resp}"))?
        .to_string();

    // Run each conversation turn
    for turn in &sample.turns {
        let _: Value = http
            .post(format!("{gateway_url}/v1/agent/completions"))
            .header("X-Tenant-ID", tenant_id)
            .json(&json!({
                "thread_id": thread_id,
                "messages": [{"role": "user", "content": turn}],
            }))
            .send()
            .await?
            .json()
            .await?;
    }

    // Ask the recall question
    let recall_resp: Value = http
        .post(format!("{gateway_url}/v1/agent/completions"))
        .header("X-Tenant-ID", tenant_id)
        .json(&json!({
            "thread_id": thread_id,
            "messages": [{"role": "user", "content": sample.recall_question}],
        }))
        .send()
        .await?
        .json()
        .await?;

    let answer = recall_resp["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    let answer_lower = answer.to_lowercase();
    let hits = sample
        .expected_keywords
        .iter()
        .filter(|kw| answer_lower.contains(kw.to_lowercase().as_str()))
        .count();

    let score = if sample.expected_keywords.is_empty() {
        1.0
    } else {
        hits as f64 / sample.expected_keywords.len() as f64
    };

    Ok((answer, score))
}
