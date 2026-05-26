//! xtask — build-time tooling for the ConusAI backend workspace.
//!
//! Usage:
//!   cargo xtask capabilities lint [--dir <path>]
//!   cargo xtask capabilities lint --strict

use agent_core::capabilities::manifest::{ToolKind, ToolManifest};
use agent_core::capabilities::validator::RegisteredToolValidator;
use anyhow::Context;
use clap::{Parser, Subcommand};
use colored::Colorize;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::ValidateCapabilities(args) => validate_capabilities(args),
        Command::Capabilities { sub } => match sub {
            CapabilitiesCommand::Lint(args) => lint(args),
            CapabilitiesCommand::Validate(args) => validate_capabilities(args),
        },
    }
}

// ── CLI ───────────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "xtask", about = "ConusAI backend build tasks")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Validate capability manifests + wiring contracts
    #[command(name = "validate-capabilities")]
    ValidateCapabilities(ValidateArgs),
    /// Capability manifest tooling
    Capabilities {
        #[command(subcommand)]
        sub: CapabilitiesCommand,
    },
}

#[derive(Subcommand)]
enum CapabilitiesCommand {
    /// Validate all capability.toml files against schema and taxonomy rules
    Lint(LintArgs),
    /// Validate manifests plus runtime wiring expectations
    Validate(ValidateArgs),
}

#[derive(Parser)]
struct LintArgs {
    /// Root directory containing capability sub-folders (default: apps/backend/capabilities)
    #[arg(long, short)]
    dir: Option<PathBuf>,
    /// Exit with error code 1 even for warnings
    #[arg(long)]
    strict: bool,
}

#[derive(Parser, Clone)]
struct ValidateArgs {
    /// Root directory containing capability sub-folders (default: apps/backend/capabilities)
    #[arg(long, short)]
    dir: Option<PathBuf>,
    /// Exit with error code 1 even for warnings
    #[arg(long)]
    strict: bool,
    /// Maximum allowed .wasm module size in megabytes
    #[arg(long, default_value_t = 32)]
    max_wasm_mb: usize,
    /// Optional comma-separated MCP endpoint allowlist (default: MCP_ALLOWED_ENDPOINTS env)
    #[arg(long)]
    mcp_allowlist: Option<String>,
}

// ── Minimal TOML shape ────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
struct CapabilityToml {
    schema_version: Option<String>,
    name: Option<String>,
    namespace: Option<String>,
    category: Option<String>,
    kind: Option<String>,
    accepts: Option<Vec<AcceptEntry>>,
    emits: Option<Vec<String>>,
    chain: Option<toml::Value>,
}

/// Mirrors the runtime `AcceptSpec` deserialization — accepts both bare strings
/// (`accepts = ["application/pdf"]`) and full objects
/// (`accepts = [{ mime = "application/pdf", max_size_mb = 20 }]`).
#[derive(Debug)]
struct AcceptEntry {
    mime: String,
    #[allow(dead_code)]
    max_size_mb: Option<u32>,
}

impl<'de> serde::Deserialize<'de> for AcceptEntry {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Form {
            Bare(String),
            Full {
                mime: String,
                #[serde(default)]
                max_size_mb: Option<u32>,
            },
        }
        Ok(match Form::deserialize(d)? {
            Form::Bare(mime) => AcceptEntry {
                mime,
                max_size_mb: None,
            },
            Form::Full { mime, max_size_mb } => AcceptEntry { mime, max_size_mb },
        })
    }
}

// ── Lint ──────────────────────────────────────────────────────────────────────

/// Categories that MUST declare `accepts` and `emits`.
const IO_CATEGORIES: &[&str] = &["extract", "convert", "sense"];

/// Valid top-level namespace roots (must match first segment of namespace).
const NAMESPACE_ROOTS: &[&str] = &[
    "storage", "compute", "sense", "extract", "convert", "compose", "deliver", "plan",
];

fn lint(args: LintArgs) -> anyhow::Result<()> {
    let dir = resolve_capabilities_dir(args.dir);

    if !dir.exists() {
        anyhow::bail!("capabilities directory not found: {}", dir.display());
    }

    let paths = capability_manifest_paths(&dir)?;

    if paths.is_empty() {
        println!(
            "{}",
            "No capability.toml files found — nothing to lint.".yellow()
        );
        return Ok(());
    }

    let mut errors = 0usize;
    let mut warnings = 0usize;

    for path in &paths {
        let (e, w) = lint_one(path);
        errors += e;
        warnings += w;
    }

    println!();
    println!(
        "Scanned {} manifest(s): {} error(s), {} warning(s)",
        paths.len(),
        errors,
        warnings
    );

    if errors > 0 || (args.strict && warnings > 0) {
        std::process::exit(1);
    }

    Ok(())
}

fn validate_capabilities(args: ValidateArgs) -> anyhow::Result<()> {
    let dir = resolve_capabilities_dir(args.dir.clone());

    if !dir.exists() {
        anyhow::bail!("capabilities directory not found: {}", dir.display());
    }

    let paths = capability_manifest_paths(&dir)?;
    if paths.is_empty() {
        println!(
            "{}",
            "No capability.toml files found — nothing to validate.".yellow()
        );
        return Ok(());
    }

    let mut errors = 0usize;
    let mut warnings = 0usize;
    let mut manifests: HashMap<String, ToolManifest> = HashMap::new();

    let mcp_allowlist_raw = args
        .mcp_allowlist
        .or_else(|| std::env::var("MCP_ALLOWED_ENDPOINTS").ok())
        .unwrap_or_default();
    let mcp_allowlist: Vec<String> = mcp_allowlist_raw
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    let max_wasm_bytes = args.max_wasm_mb * 1024 * 1024;

    for path in &paths {
        let rel = path
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());

        let raw = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                println!("{} {} — cannot read: {e}", "ERROR".red().bold(), rel);
                errors += 1;
                continue;
            }
        };

        let size_report = RegisteredToolValidator::validate_manifest_size(&raw, 256 * 1024);
        for e in size_report.errors {
            println!("  {} [{}] {}", "ERROR".red().bold(), rel, e);
            errors += 1;
        }

        let report = RegisteredToolValidator::validate_manifest(&raw);
        for e in report.errors {
            println!("  {} [{}] {}", "ERROR".red().bold(), rel, e);
            errors += 1;
        }
        for w in report.warnings {
            println!("  {} [{}] {}", "WARN ".yellow().bold(), rel, w);
            warnings += 1;
        }

        let manifest = match ToolManifest::from_toml(&raw) {
            Ok(m) => m,
            Err(e) => {
                println!("  {} [{}] {}", "ERROR".red().bold(), rel, e);
                errors += 1;
                continue;
            }
        };

        manifests.insert(manifest.name.clone(), manifest.clone());

        if !manifest.enabled {
            println!(
                "  {} [{}] capability is disabled; skipping runtime checks",
                "INFO ".cyan().bold(),
                rel
            );
            continue;
        }

        match manifest.kind {
            ToolKind::Wasm => {
                let module_name = manifest.config["wasm_module"]
                    .as_str()
                    .or_else(|| manifest.config["module"].as_str());

                let Some(module_name) = module_name else {
                    println!(
                        "  {} [{}] kind=wasm requires config.wasm_module",
                        "ERROR".red().bold(),
                        rel
                    );
                    errors += 1;
                    continue;
                };

                let module_path = path
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join(module_name);
                let bytes = match std::fs::read(&module_path) {
                    Ok(b) => b,
                    Err(e) => {
                        println!(
                            "  {} [{}] cannot read wasm module {}: {e}",
                            "ERROR".red().bold(),
                            rel,
                            module_path.display()
                        );
                        errors += 1;
                        continue;
                    }
                };

                let wasm_report = RegisteredToolValidator::validate_wasm(&bytes, max_wasm_bytes);
                for e in wasm_report.errors {
                    println!("  {} [{}] {}", "ERROR".red().bold(), rel, e);
                    errors += 1;
                }
            }
            ToolKind::Mcp | ToolKind::RemoteMcp => {
                if let Some(endpoint) = manifest.config["endpoint"].as_str() {
                    if !mcp_allowlist.is_empty() {
                        let allowed = mcp_allowlist.iter().any(|entry| endpoint.contains(entry));
                        if !allowed {
                            println!(
                                "  {} [{}] MCP endpoint disallowed by allowlist: {}",
                                "ERROR".red().bold(),
                                rel,
                                endpoint
                            );
                            errors += 1;
                        }
                    }
                } else {
                    println!(
                        "  {} [{}] kind=mcp requires config.endpoint",
                        "ERROR".red().bold(),
                        rel
                    );
                    errors += 1;
                }
            }
            ToolKind::Native => {
                // Native capabilities are backed either by NativeStorageFactory
                // (name/op dispatch) or explicit runtime wiring (`backend = "job"`).
                let has_native_storage_mapping = manifest.name == "storage-workspace"
                    || manifest.name == "storage-fs"
                    || manifest.config["op"].is_string();
                let has_job_backend = manifest.config["backend"].as_str() == Some("job");

                if !has_native_storage_mapping && !has_job_backend {
                    println!(
                        "  {} [{}] native capability has no known provider mapping (expected config.op, backend=\"job\", or known storage capability)",
                        "ERROR".red().bold(),
                        rel
                    );
                    errors += 1;
                }
            }
            ToolKind::Chain | ToolKind::Docker | ToolKind::DynamicPrompt => {}
        }
    }

    // Provider → manifest coverage for explicit runtime-wired capabilities.
    for required in ["transcribe-video"] {
        if !manifests.contains_key(required) {
            println!(
                "{} missing required manifest for runtime-wired provider: {required}",
                "ERROR".red().bold(),
            );
            errors += 1;
        }
    }

    println!();
    println!(
        "Validated {} manifest(s): {} error(s), {} warning(s)",
        manifests.len(),
        errors,
        warnings
    );

    if errors > 0 || (args.strict && warnings > 0) {
        std::process::exit(1);
    }

    Ok(())
}

fn resolve_capabilities_dir(dir: Option<PathBuf>) -> PathBuf {
    dir.unwrap_or_else(|| {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        find_workspace_root(&cwd)
            .map(|r| r.join("apps/backend/capabilities"))
            .unwrap_or_else(|| cwd.join("capabilities"))
    })
}

fn capability_manifest_paths(dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let pattern = dir.join("*/capability.toml");
    let pattern_str = pattern.to_str().context("non-UTF-8 path")?;

    let paths: Vec<PathBuf> = glob::glob(pattern_str)
        .context("bad glob")?
        .filter_map(|e| e.ok())
        .collect();

    Ok(paths)
}

fn lint_one(path: &Path) -> (usize, usize) {
    let rel = path
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());

    let raw = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            println!("{} {} — cannot read: {e}", "ERROR".red().bold(), rel);
            return (1, 0);
        }
    };

    let cap: CapabilityToml = match toml::from_str(&raw) {
        Ok(c) => c,
        Err(e) => {
            println!("{} {} — TOML parse error: {e}", "ERROR".red().bold(), rel);
            return (1, 0);
        }
    };

    let mut errors = 0usize;
    let mut warnings = 0usize;

    let mut err = |msg: &str| {
        println!("  {} [{}] {}", "ERROR".red().bold(), rel, msg);
        errors += 1;
    };
    let mut warn = |msg: &str| {
        println!("  {} [{}] {}", "WARN ".yellow().bold(), rel, msg);
        warnings += 1;
    };

    // schema_version must be present and "2.0"
    match cap.schema_version.as_deref() {
        None => err("missing `schema_version`"),
        Some(v) if v != "2.0" => warn(&format!("`schema_version = \"{v}\"` — expected \"2.0\"")),
        _ => {}
    }

    // namespace must be present
    let namespace = cap.namespace.as_deref().unwrap_or("");
    if namespace.is_empty() {
        err("missing `namespace`");
    } else {
        // First segment must be a known root.
        let root = namespace.split('.').next().unwrap_or("");
        if !NAMESPACE_ROOTS.contains(&root) {
            err(&format!(
                "namespace root `{root}` is not a known taxonomy root (allowed: {})",
                NAMESPACE_ROOTS.join(", ")
            ));
        }

        // category (if present) must match the namespace root.
        if let Some(cat) = cap.category.as_deref()
            && !cat.is_empty()
            && cat != root
        {
            warn(&format!(
                "category `{cat}` does not match namespace root `{root}` (consider aligning them)"
            ));
        }
    }

    // IO categories must declare accepts and emits.
    let category = cap.category.as_deref().unwrap_or("");
    if IO_CATEGORIES.contains(&category) {
        let accepts = cap.accepts.as_deref().unwrap_or(&[]);
        if accepts.is_empty() {
            err(&format!(
                "category `{category}` requires at least one `[[accepts]]` entry"
            ));
        }
        let emits = cap.emits.as_deref().unwrap_or(&[]);
        if emits.is_empty() {
            err(&format!(
                "category `{category}` requires at least one `emits` entry"
            ));
        }
    }

    // kind = "chain" must have a [chain] block.
    if cap.kind.as_deref() == Some("chain") && cap.chain.is_none() {
        err("`kind = \"chain\"` requires a `[chain]` block");
    }

    // Warn if accepts entries have no max_size_mb (best-practice).
    if let Some(accepts) = &cap.accepts {
        for entry in accepts {
            if entry.max_size_mb.is_none() {
                warn(&format!(
                    "`accepts` entry `{}` has no `max_size_mb` limit",
                    entry.mime
                ));
            }
        }
    }

    // name must be present.
    if cap.name.as_deref().map(|s| s.is_empty()).unwrap_or(true) {
        err("missing `name`");
    }

    let status = if errors > 0 {
        format!("{} errors, {} warnings", errors, warnings)
            .red()
            .to_string()
    } else if warnings > 0 {
        format!("0 errors, {} warnings", warnings)
            .yellow()
            .to_string()
    } else {
        "OK".green().to_string()
    };

    println!("  {} [{}]", status, rel);
    (errors, warnings)
}

fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        if current.join("Cargo.toml").exists() {
            let content = std::fs::read_to_string(current.join("Cargo.toml")).ok()?;
            if content.contains("[workspace]") {
                return Some(current);
            }
        }
        if !current.pop() {
            return None;
        }
    }
}
