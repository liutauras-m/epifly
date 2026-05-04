use crate::tools::{
    card::ToolCard,
    manifest::{ToolDef, ToolKind, ToolManifest},
};
use serde_json::json;

/// Build the built-in "native-tools" ToolCard (not loaded from YAML).
pub fn builtin_tool_card() -> ToolCard {
    let manifest = ToolManifest {
        name: "native-tools".into(),
        version: "0.1.0".into(),
        description: "Built-in filesystem and Cargo tools for workspace-aware agents".into(),
        kind: ToolKind::Native,
        config: serde_json::Value::Null,
        tags: vec!["native".into(), "filesystem".into(), "cargo".into()],
        tools: vec![
            ToolDef {
                name: "read_file".into(),
                description: "Read the contents of a file inside the tenant workspace".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Relative path within the tenant workspace"
                        }
                    },
                    "required": ["path"]
                }),
            },
            ToolDef {
                name: "write_file".into(),
                description:
                    "Write content to a file inside the tenant workspace (creates dirs as needed)"
                        .into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Relative path within the tenant workspace"
                        },
                        "content": {
                            "type": "string",
                            "description": "Text content to write"
                        }
                    },
                    "required": ["path", "content"]
                }),
            },
            ToolDef {
                name: "run_cargo".into(),
                description:
                    "Run a cargo subcommand (check/test/build/clippy/fmt) in the workspace root"
                        .into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "subcommand": {
                            "type": "string",
                            "enum": ["check", "test", "build", "clippy", "fmt"],
                            "description": "Cargo subcommand to run"
                        },
                        "args": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Extra arguments, e.g. [\"--package\", \"agent-core\"]"
                        }
                    },
                    "required": ["subcommand"]
                }),
            },
        ],
        chain: None,
    };

    ToolCard::new(manifest, std::path::PathBuf::from("."))
}
