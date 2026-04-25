use super::card::CapabilityCard;
use wasmtime::{Engine, Module};

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
}

impl Default for WasmCapabilityLoader {
    fn default() -> Self {
        Self::new().expect("failed to create WASM engine")
    }
}
