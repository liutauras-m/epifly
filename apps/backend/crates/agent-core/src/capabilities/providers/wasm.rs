//! WASM capabilities, sandboxed.
//!
//! One [`WasmRuntime`] (one `wasmtime::Engine`) serves the whole process. Every
//! call runs on a blocking thread — never on the async runtime — inside a fresh
//! `Store` with:
//! - a **fuel** budget (instruction count),
//! - a **memory** ceiling (linear memory, tables and instance count),
//! - an **epoch deadline** (wall-clock timeout, enforced by a ticker thread),
//!
//! plus an outer async timeout. Compiled modules are cached by path and mtime, so
//! hot-reloaded `.wasm` files are recompiled once, not on every call.

use crate::capabilities::card::CapabilityCard;
use crate::capabilities::manifest::{ToolKind, ToolManifest};
use crate::capabilities::provider::{CapabilityFactory, CapabilityProvider};
use crate::context::tenant::TenantContext;
use async_trait::async_trait;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use wasmtime::{Config, Engine, Linker, Module, Store, StoreLimits, StoreLimitsBuilder};

/// Epoch ticker resolution.
const EPOCH_TICK: Duration = Duration::from_millis(10);

#[derive(Debug, Clone, Copy)]
pub struct WasmLimits {
    pub fuel: u64,
    pub memory_bytes: usize,
    pub timeout: Duration,
}

impl Default for WasmLimits {
    fn default() -> Self {
        Self {
            fuel: 100_000_000,
            memory_bytes: 16 * 1024 * 1024,
            timeout: Duration::from_secs(2),
        }
    }
}

pub struct WasmRuntime {
    engine: Engine,
    limits: WasmLimits,
    modules: Mutex<HashMap<PathBuf, (Option<SystemTime>, Module)>>,
}

impl WasmRuntime {
    pub fn new(limits: WasmLimits) -> anyhow::Result<Arc<Self>> {
        let mut cfg = Config::new();
        cfg.consume_fuel(true);
        cfg.epoch_interruption(true);
        let engine = Engine::new(&cfg)?;

        let ticker = engine.weak();
        std::thread::Builder::new()
            .name("wasm-epoch".into())
            .spawn(move || {
                while let Some(engine) = ticker.upgrade() {
                    engine.increment_epoch();
                    drop(engine);
                    std::thread::sleep(EPOCH_TICK);
                }
            })?;

        Ok(Arc::new(Self {
            engine,
            limits,
            modules: Mutex::new(HashMap::new()),
        }))
    }

    /// Compiled module for `path`, recompiled only when the file's mtime changes.
    /// Blocking: call from a blocking thread.
    fn module(&self, path: &Path) -> anyhow::Result<Module> {
        let mtime = std::fs::metadata(path)
            .map_err(|e| anyhow::anyhow!("WASM module {}: {e}", path.display()))?
            .modified()
            .ok();
        if let Some((cached_mtime, module)) = self.modules.lock().unwrap().get(path)
            && *cached_mtime == mtime
        {
            return Ok(module.clone());
        }
        let module = Module::from_file(&self.engine, path)
            .map_err(|e| anyhow::anyhow!("WASM compile {}: {e}", path.display()))?;
        self.modules
            .lock()
            .unwrap()
            .insert(path.to_path_buf(), (mtime, module.clone()));
        Ok(module)
    }

    fn call_i32_blocking(&self, path: &Path, func_name: &str) -> anyhow::Result<i32> {
        let module = self.module(path)?;
        let limits: StoreLimits = StoreLimitsBuilder::new()
            .memory_size(self.limits.memory_bytes)
            .instances(1)
            .memories(1)
            .tables(4)
            .table_elements(10_000)
            .build();
        let mut store = Store::new(&self.engine, limits);
        store.limiter(|l| l);
        store.set_fuel(self.limits.fuel)?;
        let ticks = (self.limits.timeout.as_millis() / EPOCH_TICK.as_millis()).max(1) as u64;
        store.set_epoch_deadline(ticks);

        let linker: Linker<StoreLimits> = Linker::new(&self.engine);
        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|e| anyhow::anyhow!("WASM instantiation failed: {e}"))?;
        let func = instance
            .get_typed_func::<(), i32>(&mut store, func_name)
            .map_err(|e| anyhow::anyhow!("exported fn '{func_name}' not found: {e}"))?;
        func.call(&mut store, ())
            .map_err(|e| anyhow::anyhow!("WASM call '{func_name}' failed: {e}"))
    }

    /// Call an exported `() -> i32` function of the module at `path`.
    pub async fn call_i32(
        self: &Arc<Self>,
        path: PathBuf,
        func_name: String,
    ) -> anyhow::Result<i32> {
        let rt = Arc::clone(self);
        let outer = self.limits.timeout + Duration::from_secs(1);
        let task = tokio::task::spawn_blocking(move || rt.call_i32_blocking(&path, &func_name));
        match tokio::time::timeout(outer, task).await {
            Ok(joined) => joined.map_err(|e| anyhow::anyhow!("WASM task failed: {e}"))?,
            Err(_) => anyhow::bail!("WASM call timed out after {outer:?}"),
        }
    }
}

pub struct WasmProvider {
    card: CapabilityCard,
    runtime: Arc<WasmRuntime>,
}

impl WasmProvider {
    pub fn new(card: CapabilityCard, runtime: Arc<WasmRuntime>) -> Self {
        Self { card, runtime }
    }
}

#[async_trait]
impl CapabilityProvider for WasmProvider {
    fn manifest(&self) -> &ToolManifest {
        &self.card.manifest
    }

    async fn invoke(
        &self,
        tool_name: &str,
        _input: &Value,
        _tenant: Option<&TenantContext>,
    ) -> anyhow::Result<Value> {
        match tool_name {
            "ping" => {
                let path = self.card.source_dir.join("capability.wasm");
                let result = self.runtime.call_i32(path, "ping".into()).await?;
                Ok(json!({
                    "result": result,
                    "tool": self.card.manifest.name,
                    "function": tool_name,
                    "runtime": "wasmtime"
                }))
            }
            other => anyhow::bail!("unknown WASM tool: {other}"),
        }
    }
}

/// Factory for `ToolKind::Wasm`; all providers share one runtime.
pub struct WasmFactory {
    runtime: Arc<WasmRuntime>,
}

impl WasmFactory {
    pub fn new(runtime: Arc<WasmRuntime>) -> Self {
        Self { runtime }
    }
}

impl CapabilityFactory for WasmFactory {
    fn supports(&self, kind: &ToolKind, _name: &str) -> bool {
        matches!(kind, ToolKind::Wasm)
    }

    fn create(&self, card: CapabilityCard) -> anyhow::Result<Arc<dyn CapabilityProvider>> {
        Ok(Arc::new(WasmProvider::new(card, Arc::clone(&self.runtime))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_module(dir: &Path, wat: &str) -> PathBuf {
        let path = dir.join("capability.wasm");
        std::fs::write(&path, wat::parse_str(wat).unwrap()).unwrap();
        path
    }

    #[tokio::test]
    async fn returns_value_and_caches_module() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_module(
            dir.path(),
            r#"(module (func (export "ping") (result i32) i32.const 42))"#,
        );
        let rt = WasmRuntime::new(WasmLimits::default()).unwrap();
        assert_eq!(rt.call_i32(path.clone(), "ping".into()).await.unwrap(), 42);
        assert_eq!(rt.call_i32(path, "ping".into()).await.unwrap(), 42);
        assert_eq!(rt.modules.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn infinite_loop_is_stopped() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_module(
            dir.path(),
            r#"(module (func (export "ping") (result i32) (loop $l (br $l)) i32.const 0))"#,
        );
        // Huge fuel so the epoch deadline (wall clock) is what stops it.
        let rt = WasmRuntime::new(WasmLimits {
            fuel: u64::MAX,
            memory_bytes: 1024 * 1024,
            timeout: Duration::from_millis(200),
        })
        .unwrap();
        let started = std::time::Instant::now();
        assert!(rt.call_i32(path, "ping".into()).await.is_err());
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[tokio::test]
    async fn fuel_exhaustion_traps() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_module(
            dir.path(),
            r#"(module (func (export "ping") (result i32) (loop $l (br $l)) i32.const 0))"#,
        );
        let rt = WasmRuntime::new(WasmLimits {
            fuel: 10_000,
            memory_bytes: 1024 * 1024,
            timeout: Duration::from_secs(30),
        })
        .unwrap();
        let err = rt.call_i32(path, "ping".into()).await.unwrap_err();
        assert!(err.to_string().contains("failed"), "{err}");
    }

    #[tokio::test]
    async fn memory_ceiling_is_enforced() {
        let dir = tempfile::tempdir().unwrap();
        // Declares 64 pages (4 MiB) of initial memory; the ceiling is 1 MiB.
        let path = write_module(
            dir.path(),
            r#"(module (memory 64) (func (export "ping") (result i32) i32.const 1))"#,
        );
        let rt = WasmRuntime::new(WasmLimits {
            fuel: 1_000_000,
            memory_bytes: 1024 * 1024,
            timeout: Duration::from_secs(5),
        })
        .unwrap();
        assert!(rt.call_i32(path, "ping".into()).await.is_err());
    }
}
