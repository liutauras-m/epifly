//! Shared LLM chain executor — the stateless core of both `PromptChainCapability`
//! and `DynamicPromptCapability`.
//!
//! Takes a `LlmChainConfig` and a render context (JSON), renders the prompt template,
//! calls the LLM, and returns a JSON value.
//!
//! When `cfg.vision = true` the executor reads `input.image_path` from the context,
//! downloads or reads the file, base64-encodes it, and sends it as vision content
//! alongside the text prompt. All LLM calls go through `LlmRegistry` — never through
//! `rig::providers::*::Client` directly.

use crate::capabilities::executor::resolve_image_path;
use crate::capabilities::manifest::LlmChainConfig;
use crate::context::tenant::TenantContext;
use crate::llm::LlmRegistry;
use crate::llm::types::{LlmRequest, LlmResponse};
use crate::prompt::PromptTemplate;
use base64::{Engine as _, engine::general_purpose};
use rig::OneOrMany;
use rig::completion::Message;
use rig::message::{ImageMediaType, UserContent};
use serde_json::{Value, json};
use std::sync::Arc;
use tracing::{debug, instrument};

/// Execute a prompt chain with the given config, render context, and LLM registry.
///
/// `ctx` should contain `{ "input": <tool_input>, "tenant": { ... } }`.
/// When `cfg.vision = true`, `ctx.input.image_path` must be present.
#[instrument(skip(cfg, ctx, llm, tenant), fields(model = %cfg.model, vision = cfg.vision))]
pub async fn run_chain(
    cfg: &LlmChainConfig,
    ctx: &Value,
    llm: &Arc<LlmRegistry>,
    tenant: Option<&TenantContext>,
) -> anyhow::Result<Value> {
    let prompt = PromptTemplate::new(cfg.prompt_template.clone());
    let user_message = prompt.render(ctx);

    debug!(user_message = %user_message, model = %cfg.model, "run_chain executing");

    // Resolve alias → concrete (provider, model) BEFORE building the request so
    // the upstream API receives the model id, not the alias label.
    let binding = llm
        .resolve_binding(&cfg.model, tenant)
        .map_err(|e| anyhow::anyhow!("run_chain: model resolve failed: {e}"))?;
    let provider = llm
        .resolve(&cfg.model, tenant)
        .map_err(|e| anyhow::anyhow!("run_chain: provider resolve failed: {e}"))?;

    if cfg.vision {
        return run_chain_vision(cfg, ctx, &user_message, llm, tenant).await;
    }

    let mut messages = Vec::new();
    if let Some(sys) = &cfg.system_prompt {
        messages.push(Message::System {
            content: sys.clone(),
        });
    }
    messages.push(Message::User {
        content: OneOrMany::one(UserContent::text(user_message)),
    });

    let req = LlmRequest::builder()
        .model(binding.model.clone())
        .messages(messages)
        .max_tokens(cfg.max_tokens)
        .build();

    let LlmResponse { content, .. } = provider
        .complete(req)
        .await
        .map_err(|e| anyhow::anyhow!("run_chain: LLM call failed: {e}"))?;

    // Extract JSON from the response — LLMs may add prose, planning notes, or markdown fences.
    match extract_json_object(&content) {
        Some(v) => Ok(normalize_tool_output(v)),
        None => {
            tracing::warn!(
                content_len = content.len(),
                content_prefix = &content[..content.len().min(300)],
                "run_chain: no JSON found in response — returning plain text fallback"
            );
            Ok(json!({ "result": content }))
        }
    }
}

/// Vision variant: reads image_path from ctx, base64-encodes, sends as multimodal content.
async fn run_chain_vision(
    cfg: &LlmChainConfig,
    ctx: &Value,
    user_message: &str,
    llm: &Arc<LlmRegistry>,
    tenant: Option<&TenantContext>,
) -> anyhow::Result<Value> {
    // Extract the image path from the input context.
    let image_path = ctx["input"]["image_path"]
        .as_str()
        .or_else(|| ctx["input"]["document_path"].as_str())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "run_chain_vision: vision=true but input.image_path / input.document_path is missing"
            )
        })?;

    let (temp_path, effective_path) = resolve_image_path(image_path).await?;

    let bytes = std::fs::read(&effective_path).map_err(|e| {
        anyhow::anyhow!(
            "run_chain_vision: cannot read file {:?}: {e}",
            effective_path
        )
    })?;

    // Cleanup temp download after reading.
    if let Some(ref tmp) = temp_path {
        let _ = std::fs::remove_file(tmp);
    }

    let b64 = general_purpose::STANDARD.encode(&bytes);

    // Detect media type from extension for correct MIME tag.
    let media_type = match effective_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => ImageMediaType::JPEG,
        "gif" => ImageMediaType::GIF,
        "webp" => ImageMediaType::WEBP,
        _ => ImageMediaType::PNG, // PDF and unknown → PNG wrapper (Anthropic accepts it)
    };

    // Build a multimodal message: image first, then the text instruction.
    let content = OneOrMany::many(vec![
        UserContent::image_base64(b64, Some(media_type), None),
        UserContent::text(user_message),
    ])
    .map_err(|e| anyhow::anyhow!("run_chain_vision: content build failed: {e}"))?;

    let mut messages = Vec::new();
    if let Some(sys) = &cfg.system_prompt {
        messages.push(Message::System {
            content: sys.clone(),
        });
    }
    messages.push(Message::User { content });

    // Resolve the alias to a concrete (provider, model) binding so the request
    // sent upstream contains the actual model id, not the alias string.
    // (Before this fix, `cfg.model = "smart"` was forwarded literally → 404.)
    let binding = llm
        .resolve_binding(&cfg.model, tenant)
        .map_err(|e| anyhow::anyhow!("run_chain_vision: model resolve failed: {e}"))?;
    let provider = llm
        .resolve(&cfg.model, tenant)
        .map_err(|e| anyhow::anyhow!("run_chain_vision: provider resolve failed: {e}"))?;

    let req = LlmRequest::builder()
        .model(binding.model.clone())
        .messages(messages)
        .max_tokens(cfg.max_tokens)
        .build();

    let LlmResponse { content, .. } = provider
        .complete(req)
        .await
        .map_err(|e| anyhow::anyhow!("run_chain_vision: LLM call failed: {e}"))?;

    Ok(extract_json_object(&content).unwrap_or_else(|| json!({ "result": content })))
}

fn strip_markdown_fences(s: &str) -> &str {
    let s = s.trim();
    let s = s.strip_prefix("```json").unwrap_or(s);
    let s = s.strip_prefix("```").unwrap_or(s);
    let s = s.strip_suffix("```").unwrap_or(s);
    s.trim()
}

/// Try to parse a JSON object from an LLM response that may contain prose before/after.
///
/// Strategy:
/// 1. Strip markdown fences and try direct parse (fast path — well-behaved LLMs).
/// 2. Scan for a ```json … ``` fence anywhere in the text and try that.
/// 3. Find the first `{` and the last `}` in the text and try the substring.
///
/// Returns `None` when no valid JSON object can be extracted.
fn extract_json_object(raw: &str) -> Option<serde_json::Value> {
    // Fast path — well-formed LLM response.
    let trimmed = strip_markdown_fences(raw);
    if let Ok(v) = serde_json::from_str(trimmed) {
        return Some(v);
    }

    // Scan for an embedded ```json … ``` block.
    if let Some(start) = raw.find("```json") {
        let after_fence = &raw[start + 7..]; // skip "```json"
        let end = after_fence.find("```").unwrap_or(after_fence.len());
        let candidate = after_fence[..end].trim();
        if let Ok(v) = serde_json::from_str(candidate) {
            return Some(v);
        }
    }

    // Last resort: find each `{` and try to parse the balanced JSON object starting there.
    // Scan left-to-right, try each candidate starting position.
    let bytes = raw.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b != b'{' {
            continue;
        }
        // Walk forward counting braces to find the matching `}`.
        let mut depth: i32 = 0;
        let mut in_str = false;
        let mut escape_next = false;
        let mut end_idx = None;
        for (j, &ch) in bytes[i..].iter().enumerate() {
            if escape_next {
                escape_next = false;
                continue;
            }
            if in_str {
                if ch == b'\\' {
                    escape_next = true;
                } else if ch == b'"' {
                    in_str = false;
                }
                continue;
            }
            match ch {
                b'"' => in_str = true,
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        end_idx = Some(i + j);
                        break;
                    }
                }
                _ => {}
            }
        }
        if let Some(end) = end_idx {
            let candidate = &raw[i..=end];
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(candidate) {
                // Only accept objects that look like a ToolOutput (have "content" or "artifacts").
                if let Some(obj) = v.as_object()
                    && (obj.contains_key("content") || obj.contains_key("artifacts"))
                {
                    return Some(v);
                }
            }
        }
    }

    None
}

/// Normalise the raw JSON returned by a chain LLM into the canonical ToolOutput shape.
///
/// Chain LLMs are instructed to return `{ content, artifacts, metadata }`, but may return
/// ad-hoc schemas (e.g. `{ files, notes, project }` from code-project). This function:
///
/// 1. Returns the value unchanged if it already has `content` + `artifacts` keys.
/// 2. Tries to map common alternative schemas:
///    - `files` (array of `{path, content}` or `{name, data}`) → artifacts
///    - Summary from `notes`, `project`, `summary`, `description`, or `content`
///    - Existing `metadata` is preserved
///
/// Unknown schemas are returned as-is (the ArtifactBridge handles `artifacts` being absent).
fn normalize_tool_output(v: serde_json::Value) -> serde_json::Value {
    let obj = match v.as_object() {
        Some(m) => m,
        None => return v,
    };

    // Already canonical ToolOutput?
    if obj.contains_key("content") && obj.contains_key("artifacts") {
        return v;
    }

    // Try to derive artifacts from a `files` key.
    let artifacts_from_files = obj.get("files").and_then(|files| {
        // files can be an array of objects or an object keyed by path.
        let pairs: Vec<(String, String)> = if let Some(arr) = files.as_array() {
            arr.iter()
                .filter_map(|f| {
                    let path = f
                        .get("path")
                        .or_else(|| f.get("name"))
                        .or_else(|| f.get("filename"))
                        .and_then(|s| s.as_str())?;
                    let content = f
                        .get("content")
                        .or_else(|| f.get("data"))
                        .and_then(|s| s.as_str())?;
                    Some((path.to_owned(), content.to_owned()))
                })
                .collect()
        } else if let Some(map) = files.as_object() {
            map.iter()
                .filter_map(|(path, content)| Some((path.clone(), content.as_str()?.to_owned())))
                .collect()
        } else {
            return None;
        };

        if pairs.is_empty() {
            return None;
        }

        let artifacts: Vec<serde_json::Value> = pairs
            .into_iter()
            .map(|(path, content)| {
                let mime = guess_mime(&path);
                json!({ "name": path, "mime_type": mime, "data": content })
            })
            .collect();

        Some(serde_json::Value::Array(artifacts))
    });

    if let Some(artifacts) = artifacts_from_files {
        let summary = obj
            .get("notes")
            .or_else(|| obj.get("summary"))
            .or_else(|| obj.get("description"))
            .or_else(|| obj.get("project"))
            .and_then(|s| s.as_str())
            .unwrap_or("Files generated.")
            .to_owned();

        let metadata = obj
            .get("metadata")
            .cloned()
            .unwrap_or_else(|| json!({"artifact_path_prefix": ""}));

        tracing::debug!(
            artifact_count = artifacts.as_array().map(|a| a.len()).unwrap_or(0),
            "normalize_tool_output: mapped files → artifacts"
        );

        return json!({
            "content": summary,
            "artifacts": artifacts,
            "metadata": metadata,
        });
    }

    // Cannot normalise — return as-is.
    v
}

fn guess_mime(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "ts" | "tsx" | "js" | "mjs" | "cjs" => "text/javascript",
        "svelte" => "text/plain",
        "html" => "text/html",
        "css" => "text/css",
        "json" => "application/json",
        "toml" | "yaml" | "yml" | "md" | "txt" | "gitignore" | "env" => "text/plain",
        "rs" => "text/plain",
        "py" => "text/plain",
        _ => "text/plain",
    }
}
