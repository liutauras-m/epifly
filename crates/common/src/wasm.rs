use wasmtime::{Engine, Module, Store};

pub struct WasmLoader {
    engine: Engine,
}

impl WasmLoader {
    pub fn new() -> crate::error::Result<Self> {
        let engine = Engine::default();
        Ok(Self { engine })
    }

    pub fn load_bytes(&self, bytes: &[u8]) -> crate::error::Result<Module> {
        Module::from_binary(&self.engine, bytes)
            .map_err(|e| crate::error::ConusAiError::Wasm(e.to_string()))
    }

    pub fn load_file(&self, path: &std::path::Path) -> crate::error::Result<Module> {
        Module::from_file(&self.engine, path)
            .map_err(|e| crate::error::ConusAiError::Wasm(e.to_string()))
    }

    pub fn new_store<T>(&self, data: T) -> Store<T> {
        Store::new(&self.engine, data)
    }
}

impl Default for WasmLoader {
    fn default() -> Self {
        Self::new().expect("failed to create WASM engine")
    }
}
