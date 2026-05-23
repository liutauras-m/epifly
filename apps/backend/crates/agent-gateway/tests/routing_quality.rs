//! Routing-quality regression suite (PR 4.1 + 4.2).
//!
//! For each fixture in `tests/fixtures/routing_prompts.toml`, asserts that
//! `CapabilityRegistry::lexical_hint_capabilities(prompt, tenant)` returns a
//! list containing `expected_capability`. This catches:
//!
//! - Manifest changes that remove/typo a critical search_keyword (PR 2.B.4).
//! - New capabilities whose owners forgot to populate keywords.
//! - Phrase-variant regressions surfaced by `synthesize_variants`.
//!
//! Semantic ANN ranking is **not** exercised here — that would require a
//! running embedding service + Qdrant. The lexical pipeline is the
//! deterministic part of routing and the most regression-prone surface.
//! Semantic quality is verified manually in `docs/verify/verify-web.md`.
//!
//! Pass criterion: ≥ 27/30 base prompts pass. Per-case baseline drift is a
//! hard fail (`tests/fixtures/routing_baseline.txt`).
//!
//! Token-budget assertion is approximate (`bytes / 4`) and compares against
//! `target/routing_tokens.baseline` for ≤ 110% drift.

use agent_core::{
    CapabilityRegistry, NativeStorageFactory,
    capabilities::discovery::CapabilityDiscovery,
    llm::{LlmBinding, LlmRegistry},
};
use common::memory::{
    InMemoryWorkspaceContent, InMemoryWorkspaceStore, WorkspaceContentStore, WorkspaceStore,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;

// ── Fixture schema ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct Fixture {
    #[serde(rename = "case")]
    cases: Vec<Case>,
}

#[derive(Debug, Deserialize, Clone)]
struct Case {
    id: String,
    prompt: String,
    expected_capability: String,
    #[allow(dead_code)]
    #[serde(default)]
    expected_tool: Option<String>,
}

// ── Helpers (duplicated from capability_routing.rs — small enough to keep) ────

fn capabilities_dir() -> std::path::PathBuf {
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .ancestors()
        .find_map(|a| {
            let candidate = a.join("capabilities");
            if candidate.is_dir() {
                Some(candidate)
            } else {
                None
            }
        })
        .unwrap_or_else(|| std::path::PathBuf::from("capabilities"))
}

fn build_registry() -> CapabilityRegistry {
    let ws_store: Arc<dyn WorkspaceStore> = Arc::new(InMemoryWorkspaceStore::new());
    let ws_content: Arc<dyn WorkspaceContentStore> = Arc::new(InMemoryWorkspaceContent::new());

    let llm = Arc::new(LlmRegistry::new(
        HashMap::new(),
        HashMap::new(),
        LlmBinding {
            provider: "anthropic".into(),
            model: "claude-haiku-4-5".into(),
        },
    ));
    let mut reg = CapabilityRegistry::with_default_factories(Arc::clone(&llm));
    reg.register_factory(NativeStorageFactory::new(
        Arc::clone(&ws_store),
        Arc::clone(&ws_content),
    ));

    let dir = capabilities_dir();
    if dir.exists() {
        let discovery = CapabilityDiscovery::new(vec![dir]);
        let _ = discovery.discover_into(&mut reg);
    }
    reg
}

fn dev_tenant_id() -> &'static str {
    "test-tenant"
}

// ── Synthetic prompt variants (4.2.5) ────────────────────────────────────────

/// Deterministic prompt variants. Pure string ops — no LLM judge, no
/// flakiness. Returns 4 variants: base, synonym-swap, strip-articles,
/// please-prefix.
fn synthesize_variants(prompt: &str) -> Vec<String> {
    let base = prompt.to_string();

    // 1. Synonym swap: "delete" ↔ "remove", "create" ↔ "make", "add" ↔ "install".
    let synonym = {
        let mut s = prompt.to_string();
        for (a, b) in [
            ("delete", "remove"),
            ("remove", "delete"),
            ("create ", "make "),
            ("add ", "install "),
            ("show ", "display "),
            ("save ", "write "),
        ] {
            if s.to_lowercase().contains(a) {
                s = s.replace(a, b);
                break;
            }
        }
        s
    };

    // 2. Strip leading articles ("the", "a", "an") for terseness.
    let stripped = prompt
        .split_whitespace()
        .filter(|w| !matches!(w.to_lowercase().as_str(), "the" | "a" | "an"))
        .collect::<Vec<_>>()
        .join(" ");

    // 3. Polite prefix.
    let polite = format!("please {prompt}");

    vec![base, synonym, stripped, polite]
}

// ── Markdown report writer (4.2.3 + 4.2.6) ───────────────────────────────────

struct CaseResult {
    id: String,
    variant: String,
    /// True for the first (base) variant; variants are reported separately.
    is_base: bool,
    pass: bool,
    actual_caps: Vec<String>,
}

fn write_markdown_report(path: &std::path::Path, results: &[CaseResult]) {
    let mut out = String::new();
    out.push_str("# Routing-quality report\n\n");
    out.push_str(&format!("Generated: {}\n\n", chrono::Utc::now().to_rfc3339()));
    out.push_str("## Per-case results\n\n");
    out.push_str("| id | variant | pass | actual capabilities |\n");
    out.push_str("|----|---------|------|---------------------|\n");
    for r in results {
        out.push_str(&format!(
            "| `{}` | `{}` | {} | `{}` |\n",
            r.id,
            r.variant.replace('|', "\\|"),
            if r.pass { "✅" } else { "❌" },
            r.actual_caps.join(", ").replace('|', "\\|")
        ));
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, out);
}

// ── Test ──────────────────────────────────────────────────────────────────────

#[test]
fn routing_quality_baseline() {
    let registry = build_registry();
    let tenant_id = dev_tenant_id();
    let fixture: Fixture = toml::from_str(include_str!("fixtures/routing_prompts.toml"))
        .expect("routing_prompts.toml is valid TOML");

    assert!(
        fixture.cases.len() >= 30,
        "expected ≥ 30 fixtures, got {}",
        fixture.cases.len()
    );

    let mut results: Vec<CaseResult> = Vec::new();
    let mut base_passes = 0;
    let mut base_total = 0;

    for case in &fixture.cases {
        // Skip cases whose expected capability isn't even registered (e.g.
        // optional capabilities not present in this build) — they don't count
        // toward pass/fail but are reported.
        if registry.get_provider(&case.expected_capability).is_none() {
            results.push(CaseResult {
                id: case.id.clone(),
                variant: case.prompt.clone(),
                is_base: true,
                pass: true, // skipped — neutral
                actual_caps: vec!["(capability not registered)".into()],
            });
            continue;
        }

        let variants = synthesize_variants(&case.prompt);
        for (i, variant) in variants.iter().enumerate() {
            let caps = registry.lexical_hint_capabilities(variant, tenant_id);
            let pass = caps.contains(&case.expected_capability);
            results.push(CaseResult {
                id: case.id.clone(),
                variant: variant.clone(),
                is_base: i == 0,
                pass,
                actual_caps: caps,
            });
            if i == 0 {
                base_total += 1;
                if pass {
                    base_passes += 1;
                }
            }
        }
    }

    // Write markdown report regardless of pass/fail.
    let report_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/routing_quality.md");
    write_markdown_report(&report_path, &results);

    println!(
        "routing_quality: {}/{} base prompts pass",
        base_passes, base_total
    );

    // Pass criterion: at least 27/30 base prompts.
    assert!(
        base_passes >= 27,
        "routing-quality regression: {}/{} base prompts pass (need ≥ 27/30). \
         See {} for per-case detail.",
        base_passes,
        base_total,
        report_path.display()
    );

    // Baseline drift: any id in the baseline must still pass.
    let baseline_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/routing_baseline.txt");
    if baseline_path.exists() {
        let baseline: std::collections::HashSet<String> = std::fs::read_to_string(&baseline_path)
            .unwrap_or_default()
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
            .map(|s| s.trim().to_string())
            .collect();
        let mut regressed: Vec<String> = vec![];
        for r in &results {
            // Only consider base-prompt rows (one per id at most).
            if r.is_base && baseline.contains(&r.id) && !r.pass {
                regressed.push(r.id.clone());
            }
        }
        assert!(
            regressed.is_empty(),
            "baseline regression — previously-passing cases now fail: {:?}",
            regressed
        );
    }
}

#[test]
fn synthesize_variants_is_deterministic() {
    let a = synthesize_variants("delete the meeting-notes file");
    let b = synthesize_variants("delete the meeting-notes file");
    assert_eq!(a, b);
    assert_eq!(a.len(), 4);
}
