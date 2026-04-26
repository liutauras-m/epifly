use super::card::CapabilityCard;
use anyhow::Context;
use serde_json::{json, Value};
use wasmtime::{Engine, Linker, Module, Store};

pub struct WasmCapabilityLoader {
    engine: Engine,
}

impl WasmCapabilityLoader {
    pub fn new() -> common::error::Result<Self> {
        Ok(Self {
            engine: Engine::default(),
        })
    }

    pub fn load(&self, card: &CapabilityCard) -> common::error::Result<Module> {
        let wasm_path = card.source_path.join("capability.wasm");
        Module::from_file(&self.engine, &wasm_path)
            .map_err(|e| common::error::ConusAiError::Wasm(e.to_string()))
    }

    /// Invoke an exported i32-returning function from a WASM capability.
    pub fn invoke_i32(&self, card: &CapabilityCard, func_name: &str) -> common::error::Result<i32> {
        let module = self.load(card)?;
        let mut store: Store<()> = Store::new(&self.engine, ());
        let linker: Linker<()> = Linker::new(&self.engine);
        let instance = linker
            .instantiate(&mut store, &module)
            .context("WASM instantiation failed")
            .map_err(|e| common::error::ConusAiError::Wasm(e.to_string()))?;

        let func = instance
            .get_typed_func::<(), i32>(&mut store, func_name)
            .context(format!("exported fn '{func_name}' not found"))
            .map_err(|e| common::error::ConusAiError::Wasm(e.to_string()))?;

        func.call(&mut store, ())
            .context("WASM call failed")
            .map_err(|e| common::error::ConusAiError::Wasm(e.to_string()))
    }

    /// Invoke a WASM capability tool and return a JSON result.
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
                    "capability": card.manifest.name,
                    "tool": tool_name,
                    "runtime": "wasmtime"
                }))
            }
            other => Err(common::error::ConusAiError::Wasm(format!(
                "unknown WASM tool: {other}"
            ))),
        }
    }
}

impl Default for WasmCapabilityLoader {
    fn default() -> Self {
        Self::new().expect("failed to create WASM engine")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::{card::CapabilityCard, manifest::CapabilityManifest};
    use std::path::PathBuf;

    #[test]
    fn test_wasm_ping() {
        let loader = WasmCapabilityLoader::new().unwrap();

        // Resolve path relative to the workspace root
        let manifest_str = r#"
name: wasm-ping
version: "0.1.0"
description: Test
kind: wasm
tags: []
tools: []
"#;
        let manifest = CapabilityManifest::from_yaml(manifest_str).unwrap();
        let capabilities_dir =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../capabilities/template-wasm");

        let card = CapabilityCard::new(manifest, capabilities_dir);

        // Only run if the WASM file exists (CI may skip this)
        if !card.source_path.join("capability.wasm").exists() {
            eprintln!("skipping WASM test: capability.wasm not found");
            return;
        }

        let result = loader.invoke_i32(&card, "ping").unwrap();
        assert_eq!(result, 42, "WASM ping should return 42");
    }
}
