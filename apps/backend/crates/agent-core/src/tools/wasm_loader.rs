use super::card::CapabilityCard;
use serde_json::{Value, json};
use wasmtime::{Engine, Linker, Module, Store};

pub struct WasmToolLoader {
    engine: Engine,
}

impl WasmToolLoader {
    pub fn new() -> common::error::Result<Self> {
        Ok(Self {
            engine: Engine::default(),
        })
    }

    pub fn load(&self, card: &CapabilityCard) -> common::error::Result<Module> {
        let wasm_path = card.source_dir.join("capability.wasm");
        Module::from_file(&self.engine, &wasm_path)
            .map_err(|e| common::error::ConusAiError::Wasm(e.to_string()))
    }

    /// Invoke an exported i32-returning function from a WASM tool.
    pub fn invoke_i32(&self, card: &CapabilityCard, func_name: &str) -> common::error::Result<i32> {
        let module = self.load(card)?;
        let mut store: Store<()> = Store::new(&self.engine, ());
        let linker: Linker<()> = Linker::new(&self.engine);
        let instance = linker.instantiate(&mut store, &module).map_err(|e| {
            common::error::ConusAiError::Wasm(format!("WASM instantiation failed: {e}"))
        })?;

        let func = instance
            .get_typed_func::<(), i32>(&mut store, func_name)
            .map_err(|e| {
                common::error::ConusAiError::Wasm(format!(
                    "exported fn '{func_name}' not found: {e}"
                ))
            })?;

        func.call(&mut store, ())
            .map_err(|e| common::error::ConusAiError::Wasm(format!("WASM call failed: {e}")))
    }

    /// Invoke a WASM tool and return a JSON result.
    pub fn invoke_tool(
        &self,
        card: &CapabilityCard,
        tool_name: &str,
        _input: &Value,
    ) -> common::error::Result<Value> {
        match tool_name {
            "ping" => {
                let result = self.invoke_i32(card, "ping")?;
                Ok(json!({
                    "result": result,
                    "tool": card.manifest.name,
                    "function": tool_name,
                    "runtime": "wasmtime"
                }))
            }
            other => Err(common::error::ConusAiError::Wasm(format!(
                "unknown WASM tool: {other}"
            ))),
        }
    }
}

impl Default for WasmToolLoader {
    fn default() -> Self {
        Self::new().expect("failed to create WASM engine")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{card::CapabilityCard, manifest::ToolManifest};
    use std::path::PathBuf;

    #[test]
    fn test_wasm_ping() {
        let loader = WasmToolLoader::new().unwrap();

        let manifest_str = r#"
name = "wasm-ping"
version = "0.1.0"
description = "Test"
kind = "wasm"
tags = []
tools = []
"#;
        let manifest = ToolManifest::from_toml(manifest_str).unwrap();
        let capabilities_dir =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../capabilities/template-wasm");

        let card = CapabilityCard::new(manifest, capabilities_dir);

        // Only run if the WASM file exists (CI may skip this)
        if !card.source_dir.join("capability.wasm").exists() {
            eprintln!("skipping WASM test: capability.wasm not found");
            return;
        }

        let result = loader.invoke_i32(&card, "ping").unwrap();
        assert_eq!(result, 42, "WASM ping should return 42");
    }
}
