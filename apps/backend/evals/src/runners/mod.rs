use anyhow::Result;
use std::path::PathBuf;

pub mod generic;

pub use generic::ScorerKind;

pub async fn run_suite(suite: &str, dataset: Option<PathBuf>, model: &str) -> Result<()> {
    run_suite_with_scorer(suite, dataset, model, ScorerKind::Default).await
}

pub async fn run_suite_with_scorer(
    suite: &str,
    dataset: Option<PathBuf>,
    model: &str,
    scorer: ScorerKind,
) -> Result<()> {
    use generic::{EvalSample, Scorer, json_content_extractor, text_content_extractor, run_suite_with_override};

    match suite {
        "invoice" => {
            run_suite_with_override(
                dataset,
                "evals/suites/invoice.jsonl",
                model,
                |sample| {
                    let cap = sample.input["capability"].as_str().unwrap_or("extract.fields.invoice");
                    let path = sample.input["image_path"].as_str().unwrap_or("[missing]");
                    format!("Use the {cap} capability on: {path}")
                },
                json_content_extractor,
                Scorer::FieldDiff,
                "invoice extraction eval",
                Some(scorer),
            )
            .await
        }
        "ocr" | "ocr_quality" => {
            run_suite_with_override(
                dataset,
                "evals/suites/ocr.jsonl",
                model,
                |sample| {
                    let cap = sample.input["capability"].as_str().unwrap_or("extract.text.ocr_vision");
                    let path = sample.input["image_path"].as_str().unwrap_or("[missing]");
                    format!("Use the {cap} capability on: {path}")
                },
                text_content_extractor,
                Scorer::Snippets,
                "OCR quality eval",
                Some(scorer),
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
                Scorer::FieldDiff,
                "smoke eval",
                Some(scorer),
            )
            .await
        }
        "all" => {
            run_suite_with_override(
                None, "evals/suites/invoice.jsonl", model,
                |sample| {
                    let cap = sample.input["capability"].as_str().unwrap_or("extract.fields.invoice");
                    let path = sample.input["image_path"].as_str().unwrap_or("[missing]");
                    format!("Use the {cap} capability on: {path}")
                },
                json_content_extractor, Scorer::FieldDiff, "invoice extraction eval", None,
            ).await.ok();
            run_suite_with_override(
                None, "evals/suites/ocr.jsonl", model,
                |sample| {
                    let cap = sample.input["capability"].as_str().unwrap_or("extract.text.ocr_vision");
                    let path = sample.input["image_path"].as_str().unwrap_or("[missing]");
                    format!("Use the {cap} capability on: {path}")
                },
                text_content_extractor, Scorer::Snippets, "OCR quality eval", None,
            ).await.ok();
            Ok(())
        }
        other => anyhow::bail!("unknown suite: {other}. Run `evals list` to see available suites."),
    }
}
