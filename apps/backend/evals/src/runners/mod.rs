use anyhow::Result;
use std::path::PathBuf;

pub mod generic;

pub use generic::ScorerKind;

pub async fn run_suite_with_scorer(
    suite: &str,
    dataset: Option<PathBuf>,
    model: &str,
    scorer: ScorerKind,
) -> Result<()> {
    use generic::{
        Scorer, SuiteRunConfig, json_content_extractor, run_suite_with_override,
        text_content_extractor,
    };

    match suite {
        "invoice" => {
            run_suite_with_override(SuiteRunConfig {
                dataset,
                default_dataset: "evals/suites/invoice.jsonl",
                model: model.to_string(),
                prompt_fn: |sample| {
                    let cap = sample.input["capability"]
                        .as_str()
                        .unwrap_or("extract.fields.invoice");
                    let path = sample.input["image_path"].as_str().unwrap_or("[missing]");
                    format!("Use the {cap} capability on: {path}")
                },
                extract_fn: json_content_extractor,
                default_scorer: Scorer::FieldDiff,
                suite_label: "invoice extraction eval",
                scorer_override: Some(scorer),
            })
            .await
        }
        "ocr" | "ocr_quality" => {
            run_suite_with_override(SuiteRunConfig {
                dataset,
                default_dataset: "evals/suites/ocr.jsonl",
                model: model.to_string(),
                prompt_fn: |sample| {
                    let cap = sample.input["capability"]
                        .as_str()
                        .unwrap_or("extract.text.ocr_vision");
                    let path = sample.input["image_path"].as_str().unwrap_or("[missing]");
                    format!("Use the {cap} capability on: {path}")
                },
                extract_fn: text_content_extractor,
                default_scorer: Scorer::Snippets,
                suite_label: "OCR quality eval",
                scorer_override: Some(scorer),
            })
            .await
        }
        "smoke" => {
            run_suite_with_override(SuiteRunConfig {
                dataset,
                default_dataset: "evals/suites/smoke.jsonl",
                model: model.to_string(),
                prompt_fn: |sample| {
                    let cap = sample.input["capability"].as_str().unwrap_or("unknown");
                    let path = sample.input["image_path"]
                        .as_str()
                        .or_else(|| sample.input["file_path"].as_str())
                        .unwrap_or("[missing]");
                    format!("Use the {cap} capability on: {path}")
                },
                extract_fn: json_content_extractor,
                default_scorer: Scorer::FieldDiff,
                suite_label: "smoke eval",
                scorer_override: Some(scorer),
            })
            .await
        }
        "all" => {
            run_suite_with_override(SuiteRunConfig {
                dataset: None,
                default_dataset: "evals/suites/invoice.jsonl",
                model: model.to_string(),
                prompt_fn: |sample| {
                    let cap = sample.input["capability"]
                        .as_str()
                        .unwrap_or("extract.fields.invoice");
                    let path = sample.input["image_path"].as_str().unwrap_or("[missing]");
                    format!("Use the {cap} capability on: {path}")
                },
                extract_fn: json_content_extractor,
                default_scorer: Scorer::FieldDiff,
                suite_label: "invoice extraction eval",
                scorer_override: None,
            })
            .await
            .ok();
            run_suite_with_override(SuiteRunConfig {
                dataset: None,
                default_dataset: "evals/suites/ocr.jsonl",
                model: model.to_string(),
                prompt_fn: |sample| {
                    let cap = sample.input["capability"]
                        .as_str()
                        .unwrap_or("extract.text.ocr_vision");
                    let path = sample.input["image_path"].as_str().unwrap_or("[missing]");
                    format!("Use the {cap} capability on: {path}")
                },
                extract_fn: text_content_extractor,
                default_scorer: Scorer::Snippets,
                suite_label: "OCR quality eval",
                scorer_override: None,
            })
            .await
            .ok();
            Ok(())
        }
        other => anyhow::bail!("unknown suite: {other}. Run `evals list` to see available suites."),
    }
}
