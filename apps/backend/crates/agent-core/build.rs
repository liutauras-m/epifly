// Compile-time invariant: no code outside src/llm/providers/ may construct
// rig provider clients directly. Every model call must go through LlmRegistry.
//
// Fails the build with a clear message if any source file in src/ (excluding
// src/llm/providers/) contains "rig::providers::".

use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=src");

    let src = Path::new("src");
    if !src.exists() {
        return;
    }

    let violations = scan_for_violations(src);
    if !violations.is_empty() {
        eprintln!();
        eprintln!("==========================================================");
        eprintln!("BUILD ERROR: rig::providers:: bypass detected");
        eprintln!("==========================================================");
        eprintln!();
        eprintln!("All LLM calls must go through LlmRegistry::resolve_binding.");
        eprintln!("Direct rig provider construction is only allowed inside");
        eprintln!("  src/llm/providers/");
        eprintln!();
        eprintln!("Violations found:");
        for v in &violations {
            eprintln!("  {v}");
        }
        eprintln!();
        eprintln!("Fix: use llm_registry.resolve(&model_alias, tenant)?.complete(req).await");
        eprintln!("==========================================================");
        std::process::exit(1);
    }
}

fn scan_for_violations(dir: &Path) -> Vec<String> {
    let mut violations = Vec::new();
    scan_dir(dir, dir, &mut violations);
    violations
}

fn scan_dir(root: &Path, dir: &Path, violations: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            // Skip src/llm/providers/ — this is the only allowed location.
            let relative = path.strip_prefix(root).unwrap_or(&path);
            if relative == Path::new("llm/providers") {
                continue;
            }
            scan_dir(root, &path, violations);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            let relative = path.strip_prefix(root).unwrap_or(&path);
            // Skip files inside llm/providers/ (strip_prefix above skips the dir,
            // but explicit check avoids edge cases with nested paths).
            if relative
                .components()
                .collect::<Vec<_>>()
                .windows(2)
                .any(|w| {
                    let a = w[0].as_os_str().to_str().unwrap_or("");
                    let b = w[1].as_os_str().to_str().unwrap_or("");
                    a == "llm" && b == "providers"
                })
            {
                continue;
            }

            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };

            for (line_no, line) in content.lines().enumerate() {
                // Allow commented-out references.
                let trimmed = line.trim_start();
                if trimmed.starts_with("//") {
                    continue;
                }
                if line.contains("rig::providers::") {
                    violations.push(format!(
                        "{}:{}: {}",
                        relative.display(),
                        line_no + 1,
                        trimmed.chars().take(100).collect::<String>()
                    ));
                }
            }
        }
    }
}
