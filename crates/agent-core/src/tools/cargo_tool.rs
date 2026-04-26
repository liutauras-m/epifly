use anyhow::Result;
use serde_json::{json, Value};
use tokio::process::Command;

/// Allowed cargo subcommands to prevent abuse.
const ALLOWED_SUBCOMMANDS: &[&str] = &["check", "test", "build", "clippy", "fmt"];

pub async fn run_cargo(workspace_root: &str, input: &Value) -> Result<Value> {
    let subcommand = input["subcommand"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing required field: subcommand"))?;

    if !ALLOWED_SUBCOMMANDS.contains(&subcommand) {
        anyhow::bail!(
            "subcommand '{subcommand}' is not allowed; permitted: {}",
            ALLOWED_SUBCOMMANDS.join(", ")
        );
    }

    // Optional extra args (e.g. "--package agent-core")
    let extra_args: Vec<&str> = input["args"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    let mut cmd = Command::new("cargo");
    cmd.arg(subcommand)
        .args(&extra_args)
        .current_dir(workspace_root)
        // Prevent cargo from opening a pager
        .env("CARGO_TERM_COLOR", "never");

    let output = cmd
        .output()
        .await
        .map_err(|e| anyhow::anyhow!("cargo spawn failed: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let success = output.status.success();

    Ok(json!({
        "success": success,
        "exit_code": output.status.code(),
        "stdout": stdout,
        "stderr": stderr,
    }))
}
