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
        Scorer, json_content_extractor, resolve_scorer, run_suite_with_override,
        text_content_extractor,
    };

    match suite {
        "invoice" => {
            run_suite_with_override(
                dataset,
                "evals/suites/invoice.jsonl",
                model,
                |sample| {
                    let cap = sample.input["capability"]
                        .as_str()
                        .unwrap_or("extract.fields.invoice");
                    let path = sample.input["image_path"].as_str().unwrap_or("[missing]");
                    format!("Use the {cap} capability on: {path}")
                },
                json_content_extractor,
                resolve_scorer(Scorer::FieldDiff, Some(scorer)),
                "invoice extraction eval",
            )
            .await
        }
        "ocr" | "ocr_quality" => {
            run_suite_with_override(
                dataset,
                "evals/suites/ocr.jsonl",
                model,
                |sample| {
                    let cap = sample.input["capability"]
                        .as_str()
                        .unwrap_or("extract.text.ocr_vision");
                    let path = sample.input["image_path"].as_str().unwrap_or("[missing]");
                    format!("Use the {cap} capability on: {path}")
                },
                text_content_extractor,
                resolve_scorer(Scorer::Snippets, Some(scorer)),
                "OCR quality eval",
            )
            .await
        }
        "smoke" => {
            run_suite_with_override(
                dataset,
                "evals/suites/smoke.jsonl",
                model,
                |sample| {
                    let cap = sample.input["capability"].as_str().unwrap_or("unknown");
                    let path = sample.input["image_path"]
                        .as_str()
                        .or_else(|| sample.input["file_path"].as_str())
                        .unwrap_or("[missing]");
                    format!("Use the {cap} capability on: {path}")
                },
                json_content_extractor,
                resolve_scorer(Scorer::FieldDiff, Some(scorer)),
                "smoke eval",
            )
            .await
        }
        "all" => {
            run_suite_with_override(
                None,
                "evals/suites/invoice.jsonl",
                model,
                |sample| {
                    let cap = sample.input["capability"]
                        .as_str()
                        .unwrap_or("extract.fields.invoice");
                    let path = sample.input["image_path"].as_str().unwrap_or("[missing]");
                    format!("Use the {cap} capability on: {path}")
                },
                json_content_extractor,
                resolve_scorer(Scorer::FieldDiff, None),
                "invoice extraction eval",
            )
            .await
            .ok();
            run_suite_with_override(
                None,
                "evals/suites/ocr.jsonl",
                model,
                |sample| {
                    let cap = sample.input["capability"]
                        .as_str()
                        .unwrap_or("extract.text.ocr_vision");
                    let path = sample.input["image_path"].as_str().unwrap_or("[missing]");
                    format!("Use the {cap} capability on: {path}")
                },
                text_content_extractor,
                resolve_scorer(Scorer::Snippets, None),
                "OCR quality eval",
            )
            .await
            .ok();
            Ok(())
        }
        other => anyhow::bail!("unknown suite: {other}. Run `evals list` to see available suites."),
    }
}
