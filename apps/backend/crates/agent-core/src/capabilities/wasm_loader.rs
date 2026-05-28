use super::card::CapabilityCard;
use serde_json::{Value, json};
use wasmtime::{Engine, Linker, Module, ResourceLimiter, Store};

/// Maximum WASM fuel per invocation (~1 billion instructions at default fuel cost).
/// Prevents runaway WASM modules from consuming unbounded CPU.
const MAX_FUEL: u64 = 1_000_000_000;

/// Maximum linear memory a WASM module may allocate (16 MiB).
const MAX_MEMORY_BYTES: usize = 16 * 1024 * 1024;

/// Maximum table entries (function-pointer or externref tables).
const MAX_TABLE_ENTRIES: u32 = 100_000;

/// Per-invocation resource limiter — enforces memory and table caps.
struct WasmLimits;

impl ResourceLimiter for WasmLimits {
    fn memory_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        Ok(desired <= MAX_MEMORY_BYTES)
    }

    fn table_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        Ok(desired <= MAX_TABLE_ENTRIES as usize)
    }
}

pub struct WasmToolLoader {
    engine: Engine,
}

impl WasmToolLoader {
    pub fn new() -> common::error::Result<Self> {
        let mut config = wasmtime::Config::new();
        // Enable fuel-based instruction counting so `store.set_fuel()` works.
        config.consume_fuel(true);
        let engine = Engine::new(&config)
            .map_err(|e| common::error::ConusAiError::Wasm(format!("WASM engine init: {e}")))?;
        Ok(Self { engine })
    }

    /// Load a WASM module asynchronously using `tokio::fs::read` + `Module::from_binary`.
    ///
    /// `Module::from_file` is synchronous blocking I/O; this version is non-blocking.
    pub async fn load(&self, card: &CapabilityCard) -> common::error::Result<Module> {
        let wasm_path = card.source_dir.join("capability.wasm");
        let bytes = tokio::fs::read(&wasm_path).await.map_err(|e| {
            common::error::ConusAiError::Wasm(format!(
                "failed to read {}: {e}",
                wasm_path.display()
            ))
        })?;
        Module::from_binary(&self.engine, &bytes)
            .map_err(|e| common::error::ConusAiError::Wasm(e.to_string()))
    }

    /// Invoke an exported i32-returning function from a WASM tool.
    pub async fn invoke_i32(
        &self,
        card: &CapabilityCard,
        func_name: &str,
    ) -> common::error::Result<i32> {
        let module = self.load(card).await?;
        let mut store: Store<WasmLimits> = Store::new(&self.engine, WasmLimits);
        // Apply fuel cap — stops runaway modules after ~1B instructions.
        store
            .set_fuel(MAX_FUEL)
            .map_err(|e| common::error::ConusAiError::Wasm(format!("set_fuel: {e}")))?;
        // Register the memory/table resource limiter.
        store.limiter(|state| state as &mut dyn ResourceLimiter);

        let linker: Linker<WasmLimits> = Linker::new(&self.engine);
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
    pub async fn invoke_tool(
        &self,
        card: &CapabilityCard,
        tool_name: &str,
        _input: &Value,
    ) -> common::error::Result<Value> {
        match tool_name {
            "ping" => {
                let result = self.invoke_i32(card, "ping").await?;
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
    use crate::capabilities::{card::CapabilityCard, manifest::ToolManifest};
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_wasm_ping() {
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
        let capabilities_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/capabilities/template-wasm");

        let card = CapabilityCard::new(manifest, capabilities_dir);

        // Only run if the WASM file exists (CI may skip this)
        if !card.source_dir.join("capability.wasm").exists() {
            eprintln!("skipping WASM test: capability.wasm not found");
            return;
        }

        let result = loader.invoke_i32(&card, "ping").await.unwrap();
        assert_eq!(result, 42, "WASM ping should return 42");
    }

    /// Verify the engine accepts the fuel-enabled config without panicking.
    #[test]
    fn wasm_tool_loader_new_succeeds() {
        WasmToolLoader::new().expect("WasmToolLoader::new must succeed");
    }
}
