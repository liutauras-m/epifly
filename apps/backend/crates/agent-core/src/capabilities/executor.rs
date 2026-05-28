use super::card::CapabilityCard;
use super::manifest::ToolManifest;
use super::registry::CapabilityRegistry;
use crate::context::tenant::TenantContext;
use crate::llm::LlmRegistry;
use crate::llm::types::LlmRequest;
use crate::realtime::{RealtimeService, WorkspaceChangeEvent};
use common::metrics;
use parking_lot::RwLock;
use rig::completion::Message;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tracing::{Span, info, instrument, warn};

/// Public executor — dispatches through the provider registry.
pub struct ToolExecutor;

impl ToolExecutor {
    /// Dispatch a tool call through the provider registered for `cap_name`.
    #[instrument(
        skip(registry, input, tenant),
        fields(
            tool.cap    = cap_name,
            tool.name   = tool_name,
            tenant_id   = tenant.map(|t| t.tenant_id.as_str()).unwrap_or("none"),
            error.type  = tracing::field::Empty,
        )
    )]
    pub async fn invoke(
        registry: &CapabilityRegistry,
        cap_name: &str,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        let t0 = Instant::now();
        let labels = [
            metrics::kv("capability", cap_name),
            metrics::kv("tool", tool_name),
        ];
        metrics::tool_invocations().add(1, &labels);

        let result = Self::dispatch(registry, cap_name, tool_name, input, tenant).await;

        let elapsed = t0.elapsed().as_secs_f64() * 1000.0;
        metrics::tool_duration_ms().record(elapsed, &labels);

        if let Err(ref e) = result {
            metrics::tool_errors().add(1, &labels);
            metrics::record_error(&Span::current(), e);
        }

        result
    }

    async fn dispatch(
        registry: &CapabilityRegistry,
        cap_name: &str,
        tool_name: &str,
        input: &Value,
        tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        let provider = registry
            .get_provider(cap_name)
            .ok_or_else(|| anyhow::anyhow!("No provider registered for '{cap_name}'"))?;
        provider.invoke(tool_name, input, tenant).await
    }

    /// Build Anthropic-format tool definitions from a tool card (for callers
    /// that still snapshot cards without going through the provider).
    pub fn tool_definitions(card: &CapabilityCard) -> Vec<Value> {
        tool_definitions_from_manifest(&card.manifest)
    }
}

/// Shared helper used by `ToolExecutor::tool_definitions` and the default
/// `CapabilityProvider::tool_definitions` impl.
///
/// # Name sanitisation
/// Anthropic's tool-name constraint is `^[a-zA-Z0-9_-]{1,128}$`.
/// Capability names can contain dots (e.g. `media.time.current-time`) which
/// are not allowed.  We replace every `.` with `_` in the capability-name
/// prefix so the combined `{cap}__{tool}` string is always valid.
/// The inverse (replacing `_` → `.`) is applied in `SemanticCapabilityRouter::invoke`
/// as a fallback when an exact registry lookup misses.
pub fn tool_definitions_from_manifest(manifest: &ToolManifest) -> Vec<Value> {
    // Replace dots so the name satisfies Anthropic's ^[a-zA-Z0-9_-]{1,128}$ constraint.
    let safe_cap = manifest.name.replace('.', "_");
    manifest
        .tools
        .iter()
        .map(|t| {
            json!({
                "name": format!("{safe_cap}__{}", t.name),
                "description": t.description,
                "input_schema": t.input_schema
            })
        })
        .collect()
}

// ── Plan executor ─────────────────────────────────────────────────────────────

/// Outcome of a single plan step.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StepResult {
    pub step_idx: usize,
    pub capability: String,
    pub tool: String,
    pub strategy: String,
    pub output: Option<Value>,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub tokens_in: Option<u32>,
    pub tokens_out: Option<u32>,
    pub cost_hint_class: Option<String>,
}

/// A single plan step declared by the planner capability.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct PlanStep {
    pub capability: String,
    pub tool: String,
    pub input: Value,
    #[serde(default = "default_strategy")]
    pub strategy: String,
}

fn default_strategy() -> String {
    "single".into()
}

/// Maximum tokens the LLM reducer may produce.
const MAX_REDUCER_TOKENS: u32 = 2_048;

/// Execute a `plan_steps` array returned by the planner capability.
///
/// Strategies per step:
/// - `"single"` — invoke once and return the result.
/// - `"parallel_consensus"` — invoke the same tool twice concurrently; if the
///   results agree (serialised equality) return either; otherwise ask a cheap
///   LLM to choose.
/// - `"fallback_cascade"` — invoke; on error fall back to an echo of the input.
///
/// The registry is accepted as `Arc<Mutex<>>` so the lock can be released
/// between async invocations, preventing deadlocks in hook callbacks.
///
/// `realtime` — when provided, `pipeline.step.started` / `pipeline.step.finished`
/// events are published on the tenant's broadcast channel so the UI can render a
/// live pipeline timeline without polling.
#[instrument(skip(registry, llm, tenant, realtime, steps), fields(plan_len = steps.len()))]
pub async fn run_plan(
    steps: Vec<PlanStep>,
    registry: Arc<RwLock<CapabilityRegistry>>,
    llm: Option<Arc<LlmRegistry>>,
    tenant: Option<TenantContext>,
    realtime: Option<Arc<RealtimeService>>,
) -> Vec<StepResult> {
    let mut results = Vec::with_capacity(steps.len());

    for (idx, step) in steps.iter().enumerate() {
        let t0 = Instant::now();
        let span = tracing::info_span!(
            "plan_step",
            step_idx = idx,
            capability = %step.capability,
            tool = %step.tool,
            strategy = %step.strategy,
        );
        let _guard = span.enter();

        // Emit pipeline.step.started realtime event.
        if let (Some(rt), Some(tc)) = (realtime.as_ref(), tenant.as_ref()) {
            rt.publish_workspace_change(WorkspaceChangeEvent {
                op: "pipeline.step.started".into(),
                tenant_id: tc.tenant_id.to_string(),
                node_id: format!("{}.{}", step.capability, step.tool),
                kind: "pipeline_step".into(),
            })
            .await;
        }

        // Validate and extract metadata while holding the lock briefly.
        let validation = {
            let reg = registry.read();
            validate_step(&reg, &step.capability, &step.tool)
        };

        match validation {
            Err(e) => {
                warn!(step_idx = idx, error = %e, "plan step validation failed");
                results.push(StepResult {
                    step_idx: idx,
                    capability: step.capability.clone(),
                    tool: step.tool.clone(),
                    strategy: step.strategy.clone(),
                    output: None,
                    error: Some(e),
                    duration_ms: t0.elapsed().as_millis() as u64,
                    tokens_in: None,
                    tokens_out: None,
                    cost_hint_class: None,
                });
                continue;
            }
            Ok(cost_hint_class) => {
                let outcome = match step.strategy.as_str() {
                    "parallel_consensus" => {
                        run_parallel_consensus(
                            Arc::clone(&registry),
                            &step.capability,
                            &step.tool,
                            &step.input,
                            tenant.as_ref(),
                            llm.as_ref(),
                        )
                        .await
                    }
                    "fallback_cascade" => {
                        run_fallback_cascade(
                            Arc::clone(&registry),
                            &step.capability,
                            &step.tool,
                            &step.input,
                            tenant.as_ref(),
                        )
                        .await
                    }
                    _ => {
                        // "single" — lock briefly to get the provider, then invoke without lock.
                        let provider = {
                            let reg = registry.read();
                            reg.get_provider(&step.capability)
                        };
                        match provider {
                            Some(p) => p
                                .invoke(&step.tool, &step.input, tenant.as_ref())
                                .await
                                .map_err(|e| e.to_string()),
                            None => Err(format!("no provider for '{}'", step.capability)),
                        }
                    }
                };

                let duration_ms = t0.elapsed().as_millis() as u64;
                let (output, error) = match outcome {
                    Ok(v) => {
                        info!(step_idx = idx, duration_ms, "plan step succeeded");
                        (Some(v), None)
                    }
                    Err(e) => {
                        warn!(step_idx = idx, error = %e, duration_ms, "plan step failed");
                        (None, Some(e))
                    }
                };

                // Emit pipeline.step.finished realtime event.
                if let (Some(rt), Some(tc)) = (realtime.as_ref(), tenant.as_ref()) {
                    rt.publish_workspace_change(WorkspaceChangeEvent {
                        op: "pipeline.step.finished".into(),
                        tenant_id: tc.tenant_id.to_string(),
                        node_id: format!("{}.{}", step.capability, step.tool),
                        kind: "pipeline_step".into(),
                    })
                    .await;
                }

                results.push(StepResult {
                    step_idx: idx,
                    capability: step.capability.clone(),
                    tool: step.tool.clone(),
                    strategy: step.strategy.clone(),
                    output,
                    error,
                    duration_ms,
                    tokens_in: None,
                    tokens_out: None,
                    cost_hint_class,
                });
            }
        }
    }

    results
}

fn validate_step(
    registry: &CapabilityRegistry,
    capability: &str,
    tool: &str,
) -> Result<Option<String>, String> {
    let card = registry
        .get(capability)
        .ok_or_else(|| format!("unknown capability '{capability}'"))?;

    if !card.manifest.tools.iter().any(|t| t.name == tool) {
        return Err(format!(
            "unknown tool '{tool}' for capability '{capability}'"
        ));
    }

    let cost_hint_class = card
        .manifest
        .cost_hint
        .as_ref()
        .map(|h| h.bucket().to_string());
    Ok(cost_hint_class)
}

async fn run_parallel_consensus(
    registry: Arc<RwLock<CapabilityRegistry>>,
    cap: &str,
    tool: &str,
    input: &Value,
    tenant: Option<&TenantContext>,
    llm: Option<&Arc<LlmRegistry>>,
) -> Result<Value, String> {
    // Get the provider once — Arc<dyn CapabilityProvider> is Send + Sync.
    let provider = {
        let reg = registry.read();
        reg.get_provider(cap)
            .ok_or_else(|| format!("no provider for '{cap}'"))?
    };
    let (r1, r2) = tokio::join!(
        provider.invoke(tool, input, tenant),
        provider.invoke(tool, input, tenant),
    );
    let (r1, r2) = (r1.map_err(|e| e.to_string()), r2.map_err(|e| e.to_string()));

    match (r1, r2) {
        (Ok(a), Ok(b)) => {
            // Compare by value rather than allocating two String representations.
            if a == b {
                Ok(a)
            } else {
                // Results differ — ask cheap LLM to pick the better one.
                if let Some(llm) = llm {
                    llm_judge_consensus(llm, &a, &b, tenant).await
                } else {
                    // No LLM available — fall back to first result.
                    Ok(a)
                }
            }
        }
        (Ok(a), Err(_)) => Ok(a),
        (Err(_), Ok(b)) => Ok(b),
        (Err(e1), Err(_)) => Err(e1.to_string()),
    }
}

async fn llm_judge_consensus(
    llm: &Arc<LlmRegistry>,
    a: &Value,
    b: &Value,
    tenant: Option<&TenantContext>,
) -> Result<Value, String> {
    let provider = llm.resolve("cheap", tenant).map_err(|e| e.to_string())?;

    let prompt = format!(
        "Two agents produced different results for the same task. Choose the more accurate result.\n\
         Result A:\n{}\n\nResult B:\n{}\n\n\
         Reply with ONLY the letter A or B.",
        serde_json::to_string_pretty(a).unwrap_or_default(),
        serde_json::to_string_pretty(b).unwrap_or_default(),
    );

    let req = LlmRequest::builder()
        .model("cheap".to_string())
        .messages(vec![Message::User {
            content: rig::OneOrMany::one(rig::message::UserContent::text(prompt)),
        }])
        .max_tokens(MAX_REDUCER_TOKENS)
        .build();

    let resp = provider.complete(req).await.map_err(|e| e.to_string())?;
    if resp.content.trim().to_uppercase().starts_with('B') {
        Ok(b.clone())
    } else {
        Ok(a.clone())
    }
}

async fn run_fallback_cascade(
    registry: Arc<RwLock<CapabilityRegistry>>,
    cap: &str,
    tool: &str,
    input: &Value,
    tenant: Option<&TenantContext>,
) -> Result<Value, String> {
    let provider = {
        let reg = registry.read();
        reg.get_provider(cap)
            .ok_or_else(|| format!("no provider for '{cap}'"))?
    };
    match provider.invoke(tool, input, tenant).await {
        Ok(v) => Ok(v),
        Err(e) => {
            warn!(capability = cap, tool, error = %e, "fallback_cascade: primary failed, echoing input");
            Ok(json!({ "fallback": true, "echoed_input": input, "error": e.to_string() }))
        }
    }
}

/// If `image_path` is a URL, download it to a temp file and return `(Some(temp), temp_path)`.
/// If it's a local path, return `(None, original_path)`.
pub async fn resolve_image_path(image_path: &str) -> anyhow::Result<(Option<PathBuf>, PathBuf)> {
    // Rewrite /ui/files/{token} URLs to the gateway-internal address.
    // The frontend embeds window.location.origin (e.g. http://localhost:5175 in dev) but the
    // gateway must fetch the file through its own loopback — /ui/files supports token-only auth.
    let rewritten;
    let image_path = if let Some(path_start) = image_path.find("/ui/files/") {
        let base = std::env::var("GATEWAY_INTERNAL_URL")
            .unwrap_or_else(|_| "http://localhost:8080".to_string());
        rewritten = format!("{}{}", base, &image_path[path_start..]);
        rewritten.as_str()
    } else {
        image_path
    };

    if image_path.starts_with("http://") || image_path.starts_with("https://") {
        let bytes = reqwest::get(image_path)
            .await
            .map_err(|e| anyhow::anyhow!("download failed: {e}"))?
            .bytes()
            .await
            .map_err(|e| anyhow::anyhow!("read body failed: {e}"))?;

        let ext = if image_path.contains(".pdf") {
            "pdf"
        } else if image_path.contains(".jpg") || image_path.contains(".jpeg") {
            "jpg"
        } else {
            "png"
        };

        let tmp = std::env::temp_dir().join(format!("conusai-{}.{ext}", uuid::Uuid::new_v4()));
        std::fs::write(&tmp, &bytes).map_err(|e| anyhow::anyhow!("write temp file failed: {e}"))?;

        Ok((Some(tmp.clone()), tmp))
    } else {
        Ok((None, Path::new(image_path).to_path_buf()))
    }
}
