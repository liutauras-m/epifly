//! xtask — build-time tooling for the ConusAI backend workspace.
//!
//! Usage:
//!   cargo xtask capabilities lint [--dir <path>]
//!   cargo xtask capabilities lint --strict

use anyhow::Context;
use clap::{Parser, Subcommand};
use colored::Colorize;
use serde::Deserialize;
use std::path::{Path, PathBuf};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Capabilities { sub } => match sub {
            CapabilitiesCommand::Lint(args) => lint(args),
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

#[derive(Deserialize)]
struct AcceptEntry {
    mime: String,
    #[allow(dead_code)]
    max_size_mb: Option<u32>,
}

// ── Lint ──────────────────────────────────────────────────────────────────────

/// Categories that MUST declare `accepts` and `emits`.
const IO_CATEGORIES: &[&str] = &["extract", "convert", "sense"];

/// Valid top-level namespace roots (must match first segment of namespace).
const NAMESPACE_ROOTS: &[&str] = &[
    "storage", "compute", "sense", "extract", "convert", "compose", "deliver", "plan",
];

fn lint(args: LintArgs) -> anyhow::Result<()> {
    // Resolve the capabilities directory relative to the workspace root.
    let dir = args.dir.unwrap_or_else(|| {
        // Try to find workspace root by walking up from cwd.
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        find_workspace_root(&cwd)
            .map(|r| r.join("apps/backend/capabilities"))
            .unwrap_or_else(|| cwd.join("capabilities"))
    });

    if !dir.exists() {
        anyhow::bail!("capabilities directory not found: {}", dir.display());
    }

    let pattern = dir.join("*/capability.toml");
    let pattern_str = pattern
        .to_str()
        .context("non-UTF-8 path")?;

    let paths: Vec<PathBuf> = glob::glob(pattern_str)
        .context("bad glob")?
        .filter_map(|e| e.ok())
        .collect();

    if paths.is_empty() {
        println!("{}", "No capability.toml files found — nothing to lint.".yellow());
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
        if let Some(cat) = cap.category.as_deref() {
            if !cat.is_empty() && cat != root {
                warn(&format!(
                    "category `{cat}` does not match namespace root `{root}` (consider aligning them)"
                ));
            }
        }
    }

    // IO categories must declare accepts and emits.
    let category = cap.category.as_deref().unwrap_or("");
    if IO_CATEGORIES.contains(&category) {
        let accepts = cap.accepts.as_deref().unwrap_or(&[]);
        if accepts.is_empty() {
            err(&format!("category `{category}` requires at least one `[[accepts]]` entry"));
        }
        let emits = cap.emits.as_deref().unwrap_or(&[]);
        if emits.is_empty() {
            err(&format!("category `{category}` requires at least one `emits` entry"));
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
