//! Generic capability runner — reusable HTTP harness for all eval suites.
//!
//! Wraps the gateway `/v1/agent/completions` endpoint. The caller provides a prompt
//! template and a scorer; the runner handles batching, error capture, and reporting.
use anyhow::Result;
use serde_json::{Value, json};
use std::path::PathBuf;

use crate::report::print_report;
use crate::scorers::ScorerResult;

/// A sample from a JSONL eval dataset.
#[derive(serde::Deserialize)]
pub struct EvalSample {
    /// Human-readable label shown in progress output.
    pub label: Option<String>,
    /// Free-form input data passed to the prompt builder.
    pub input: Value,
    /// Expected output — passed as-is to the scorer.
    pub expected: Value,
}

/// Scoring strategy for a single sample.
pub enum Scorer {
    /// Exact JSON equality between extracted and expected.
    Exact,
    /// Field-by-field diff: auto-discover all keys in `expected` and score each present+matching key.
    FieldDiff,
    /// Check that each string in `snippets` appears (case-insensitive) in the text output.
    Snippets,
    /// Use a gateway LLM call (haiku) to judge the output against the expected.
    LlmJudge,
}

/// CLI scorer name as passed via `--scorer`.
#[derive(Clone, Debug)]
pub enum ScorerKind {
    Exact,
    FieldDiff,
    LlmJudge,
    Default,
}

impl std::str::FromStr for ScorerKind {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "exact" => Ok(ScorerKind::Exact),
            "field-diff" => Ok(ScorerKind::FieldDiff),
            "llm-judge" => Ok(ScorerKind::LlmJudge),
            "default" => Ok(ScorerKind::Default),
            other => anyhow::bail!(
                "unknown scorer: {other}. Allowed: exact, field-diff, llm-judge, default"
            ),
        }
    }
}

/// Run an eval suite using the gateway API.
///
/// * `prompt_fn` — builds the user message from an `EvalSample`.
/// * `extract_fn` — extracts a scoreable `Value` from the raw gateway JSON response.
/// * `default_scorer` — the suite's default scoring strategy.
/// * `scorer_override` — when `Some`, replaces the suite's default scorer (from `--scorer` CLI flag).
pub struct SuiteRunConfig<P, E>
where
    P: Fn(&EvalSample) -> String,
    E: Fn(&Value) -> Value,
{
    pub dataset: Option<PathBuf>,
    pub default_dataset: &'static str,
    pub model: String,
    pub prompt_fn: P,
    pub extract_fn: E,
    pub default_scorer: Scorer,
    pub suite_label: &'static str,
    pub scorer_override: Option<ScorerKind>,
}

pub async fn run_suite_with_override<P, E>(cfg: SuiteRunConfig<P, E>) -> Result<()>
where
    P: Fn(&EvalSample) -> String,
    E: Fn(&Value) -> Value,
{
    let scorer = match cfg.scorer_override {
        Some(ScorerKind::Exact) => Scorer::Exact,
        Some(ScorerKind::FieldDiff) => Scorer::FieldDiff,
        Some(ScorerKind::LlmJudge) => Scorer::LlmJudge,
        Some(ScorerKind::Default) | None => cfg.default_scorer,
    };
    _run_suite_inner(
        cfg.dataset,
        cfg.default_dataset,
        &cfg.model,
        cfg.prompt_fn,
        cfg.extract_fn,
        scorer,
        cfg.suite_label,
    )
    .await
}

async fn _run_suite_inner(
    dataset: Option<PathBuf>,
    default_dataset: &str,
    model: &str,
    prompt_fn: impl Fn(&EvalSample) -> String,
    extract_fn: impl Fn(&Value) -> Value,
    scorer: Scorer,
    suite_label: &str,
) -> Result<()> {
    let dataset_path = dataset.unwrap_or_else(|| PathBuf::from(default_dataset));
    anyhow::ensure!(
        dataset_path.exists(),
        "Dataset not found: {:?}",
        dataset_path
    );

    let gateway_url =
        std::env::var("GATEWAY_URL").unwrap_or_else(|_| "http://localhost:8080".into());
    let tenant_id = std::env::var("EVAL_TENANT_ID").unwrap_or_else(|_| "eval".into());

    let content = std::fs::read_to_string(&dataset_path)?;
    let samples: Vec<EvalSample> = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).map_err(anyhow::Error::from))
        .collect::<Result<_>>()?;

    println!(
        "Running {} — {} samples, model: {}, gateway: {}",
        suite_label,
        samples.len(),
        model,
        gateway_url
    );

    let http = reqwest::Client::new();
    let mut results: Vec<ScorerResult> = Vec::new();

    for (i, sample) in samples.iter().enumerate() {
        let label = sample
            .label
            .clone()
            .unwrap_or_else(|| format!("sample-{}", i + 1));
        print!("  [{}/{}] {} ... ", i + 1, samples.len(), label);

        let prompt = prompt_fn(sample);
        let resp = http
            .post(format!("{gateway_url}/v1/agent/completions"))
            .header("X-Tenant-ID", &tenant_id)
            .json(&json!({
                "model": model,
                "messages": [{ "role": "user", "content": prompt }]
            }))
            .send()
            .await;

        match resp {
            Ok(r) => match r.json::<Value>().await {
                Ok(body) => {
                    let extracted = extract_fn(&body);
                    let result = match &scorer {
                        Scorer::LlmJudge => {
                            llm_judge(
                                &http,
                                &gateway_url,
                                &tenant_id,
                                &extracted,
                                &sample.expected,
                            )
                            .await
                        }
                        s => score(s, &extracted, &sample.expected),
                    };
                    let status = if result.passed { "✅" } else { "❌" };
                    println!("{} score={:.2}", status, result.score);
                    results.push(result);
                }
                Err(e) => {
                    println!("❌ JSON parse error: {e}");
                    results.push(fail(format!("json parse: {e}")));
                }
            },
            Err(e) => {
                println!("❌ HTTP error: {e}");
                results.push(fail(format!("http: {e}")));
            }
        }
    }

    print_report(&results);
    Ok(())
}

/// LLM-based judge: sends extracted output and expected to the gateway and asks for a 0–1 score.
///
/// Uses the `haiku` model alias (cheap) via `GATEWAY_URL/v1/agent/completions`.
/// Falls back to `FieldDiff` if the gateway call fails or the response cannot be parsed.
async fn llm_judge(
    http: &reqwest::Client,
    gateway_url: &str,
    tenant_id: &str,
    extracted: &Value,
    expected: &Value,
) -> ScorerResult {
    let judge_prompt = format!(
        "You are an evaluation judge. Score how well the ACTUAL output matches the EXPECTED output.\n\n\
         EXPECTED:\n{expected}\n\nACTUAL:\n{extracted}\n\n\
         Respond with a single JSON object: {{\"score\": <0.0-1.0>, \"passed\": <true|false>, \"reason\": \"<one line>\"}}.\n\
         Score 1.0 = perfect match, 0.0 = completely wrong. Passed = score >= 0.8.\n\
         Respond ONLY with the JSON object, no markdown.",
        expected = serde_json::to_string_pretty(expected).unwrap_or_default(),
        extracted = serde_json::to_string_pretty(extracted).unwrap_or_default(),
    );

    let resp = http
        .post(format!("{gateway_url}/v1/agent/completions"))
        .header("X-Tenant-ID", tenant_id)
        .json(&json!({
            "model": "haiku",
            "messages": [{ "role": "user", "content": judge_prompt }]
        }))
        .send()
        .await;

    let body = match resp {
        Ok(r) => match r.json::<Value>().await {
            Ok(b) => b,
            Err(e) => {
                eprintln!("llm-judge: response parse error: {e}; falling back to field-diff");
                return score(&Scorer::FieldDiff, extracted, expected);
            }
        },
        Err(e) => {
            eprintln!("llm-judge: gateway error: {e}; falling back to field-diff");
            return score(&Scorer::FieldDiff, extracted, expected);
        }
    };

    let content = body["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("{}");

    // Strip markdown fences if present.
    let clean = content
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    match serde_json::from_str::<Value>(clean) {
        Ok(v) => {
            let score_val = v["score"].as_f64().unwrap_or(0.0).clamp(0.0, 1.0);
            let passed = v["passed"].as_bool().unwrap_or(score_val >= 0.8);
            let reason = v["reason"].as_str().unwrap_or("(no reason)").to_string();
            ScorerResult {
                score: score_val,
                passed,
                details: reason,
            }
        }
        Err(e) => {
            eprintln!("llm-judge: could not parse judge JSON ({e}); falling back to field-diff");
            score(&Scorer::FieldDiff, extracted, expected)
        }
    }
}

fn score(scorer: &Scorer, extracted: &Value, expected: &Value) -> ScorerResult {
    match scorer {
        Scorer::Exact => {
            let passed = extracted == expected;
            ScorerResult {
                score: if passed { 1.0 } else { 0.0 },
                passed,
                details: if passed {
                    "exact match".into()
                } else {
                    format!("expected {expected}, got {extracted}")
                },
            }
        }
        Scorer::FieldDiff => {
            let fields: Vec<String> = expected
                .as_object()
                .map(|m| m.keys().cloned().collect())
                .unwrap_or_default();
            if fields.is_empty() {
                return ScorerResult {
                    score: 1.0,
                    passed: true,
                    details: "no expected fields".into(),
                };
            }
            let mut hits = 0usize;
            let mut diffs = Vec::new();
            for field in &fields {
                match (expected.get(field), extracted.get(field)) {
                    (Some(e), Some(g)) if values_match(e, g) => hits += 1,
                    (None, _) => hits += 1,
                    (Some(e), Some(g)) => diffs.push(format!("{field}: expected={e}, got={g}")),
                    (Some(e), None) => diffs.push(format!("{field}: expected={e}, missing")),
                }
            }
            let score = hits as f64 / fields.len() as f64;
            ScorerResult {
                score,
                passed: score >= 0.8,
                details: if diffs.is_empty() {
                    "all fields match".into()
                } else {
                    diffs.join("; ")
                },
            }
        }
        Scorer::Snippets => {
            let text = extracted.as_str().unwrap_or("").to_lowercase();
            let snippets = expected
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_lowercase()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let hits = snippets
                .iter()
                .filter(|s| text.contains(s.as_str()))
                .count();
            let score = if snippets.is_empty() {
                1.0
            } else {
                hits as f64 / snippets.len() as f64
            };
            ScorerResult {
                score,
                passed: score >= 0.8,
                details: format!("{}/{} snippets found", hits, snippets.len()),
            }
        }
        Scorer::LlmJudge => score(&Scorer::FieldDiff, extracted, expected),
    }
}

fn values_match(expected: &Value, got: &Value) -> bool {
    match (expected, got) {
        (Value::String(e), Value::String(g)) => e.to_lowercase().trim() == g.to_lowercase().trim(),
        (Value::Number(e), Value::Number(g)) => {
            (e.as_f64().unwrap_or(0.0) - g.as_f64().unwrap_or(0.0)).abs() < 0.01
        }
        (Value::Null, Value::Null) => true,
        _ => expected == got,
    }
}

fn fail(details: String) -> ScorerResult {
    ScorerResult {
        score: 0.0,
        passed: false,
        details,
    }
}

/// Standard extractor: parse assistant message content as JSON, fallback to `{raw: ...}`.
pub fn json_content_extractor(body: &Value) -> Value {
    let content = body["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("{}");
    serde_json::from_str(content).unwrap_or_else(|_| json!({ "raw": content }))
}

/// Standard extractor: return assistant message content as a string value.
pub fn text_content_extractor(body: &Value) -> Value {
    let text = body["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("");
    Value::String(text.to_string())
}
