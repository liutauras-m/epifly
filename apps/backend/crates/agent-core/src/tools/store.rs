//! Filesystem persistence for capabilities.
//!
//! Trait `RegisteredToolStore` allows swapping in `InMemoryStore` for tests.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredToolState {
    pub enabled: bool,
    pub created_at: String, // RFC 3339
    pub updated_at: String,
}

/// Persistence contract for capability manifests + state.
pub trait RegisteredToolStore: Send + Sync {
    fn list(&self) -> anyhow::Result<Vec<String>>;
    fn read_manifest(&self, name: &str) -> anyhow::Result<String>;
    fn write_manifest(&self, name: &str, toml: &str) -> anyhow::Result<()>;
    fn write_wasm(&self, name: &str, bytes: &[u8]) -> anyhow::Result<()>;
    fn read_state(&self, name: &str) -> anyhow::Result<Option<RegisteredToolState>>;
    fn write_state(&self, name: &str, state: &RegisteredToolState) -> anyhow::Result<()>;
    fn delete(&self, name: &str) -> anyhow::Result<()>;
    fn capability_dir(&self, name: &str) -> PathBuf;
}

// ── Filesystem implementation ─────────────────────────────────────────────────

pub struct FilesystemStore {
    root: PathBuf,
}

impl FilesystemStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn from_env() -> Self {
        let dir = std::env::var("CONUSAI_CAPABILITIES_DIR")
            .unwrap_or_else(|_| "./capabilities".to_string());
        Self::new(dir)
    }
}

impl RegisteredToolStore for FilesystemStore {
    fn list(&self) -> anyhow::Result<Vec<String>> {
        if !self.root.exists() {
            return Ok(vec![]);
        }
        let mut names = Vec::new();
        for entry in std::fs::read_dir(&self.root)? {
            let entry = entry?;
            if entry.path().is_dir()
                && let Some(name) = entry.file_name().to_str()
                && entry.path().join("capability.toml").exists()
            {
                names.push(name.to_string());
            }
        }
        Ok(names)
    }

    fn read_manifest(&self, name: &str) -> anyhow::Result<String> {
        let path = self.capability_dir(name).join("capability.toml");
        Ok(std::fs::read_to_string(&path)?)
    }

    fn write_manifest(&self, name: &str, toml: &str) -> anyhow::Result<()> {
        let dir = self.capability_dir(name);
        std::fs::create_dir_all(&dir)?;
        let tmp = dir.join("capability.toml.tmp");
        std::fs::write(&tmp, toml)?;
        std::fs::rename(&tmp, dir.join("capability.toml"))?;
        Ok(())
    }

    fn write_wasm(&self, name: &str, bytes: &[u8]) -> anyhow::Result<()> {
        let dir = self.capability_dir(name);
        std::fs::create_dir_all(&dir)?;
        let tmp = dir.join("capability.wasm.tmp");
        std::fs::write(&tmp, bytes)?;
        std::fs::rename(&tmp, dir.join("capability.wasm"))?;
        Ok(())
    }

    fn read_state(&self, name: &str) -> anyhow::Result<Option<RegisteredToolState>> {
        let path = self.capability_dir(name).join("state.json");
        if !path.exists() {
            return Ok(None);
        }
        let raw = std::fs::read_to_string(&path)?;
        Ok(Some(serde_json::from_str(&raw)?))
    }

    fn write_state(&self, name: &str, state: &RegisteredToolState) -> anyhow::Result<()> {
        let dir = self.capability_dir(name);
        std::fs::create_dir_all(&dir)?;
        let tmp = dir.join("state.json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(state)?)?;
        std::fs::rename(&tmp, dir.join("state.json"))?;
        Ok(())
    }

    fn delete(&self, name: &str) -> anyhow::Result<()> {
        let dir = self.capability_dir(name);
        if dir.exists() {
            std::fs::remove_dir_all(&dir)?;
        }
        Ok(())
    }

    fn capability_dir(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }
}

// ── In-memory implementation (tests) ─────────────────────────────────────────

#[cfg(test)]
pub mod mem {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Default)]
    pub struct InMemoryStore {
        manifests: Mutex<HashMap<String, String>>,
        wasms: Mutex<HashMap<String, Vec<u8>>>,
        states: Mutex<HashMap<String, RegisteredToolState>>,
    }

    impl InMemoryStore {
        pub fn new() -> Self {
            Self::default()
        }
    }

    impl RegisteredToolStore for InMemoryStore {
        fn list(&self) -> anyhow::Result<Vec<String>> {
            Ok(self.manifests.lock().unwrap().keys().cloned().collect())
        }
        fn read_manifest(&self, name: &str) -> anyhow::Result<String> {
            self.manifests
                .lock()
                .unwrap()
                .get(name)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("not found: {name}"))
        }
        fn write_manifest(&self, name: &str, toml: &str) -> anyhow::Result<()> {
            self.manifests
                .lock()
                .unwrap()
                .insert(name.to_string(), toml.to_string());
            Ok(())
        }
        fn write_wasm(&self, name: &str, bytes: &[u8]) -> anyhow::Result<()> {
            self.wasms
                .lock()
                .unwrap()
                .insert(name.to_string(), bytes.to_vec());
            Ok(())
        }
        fn read_state(&self, name: &str) -> anyhow::Result<Option<RegisteredToolState>> {
            Ok(self.states.lock().unwrap().get(name).cloned())
        }
        fn write_state(&self, name: &str, state: &RegisteredToolState) -> anyhow::Result<()> {
            self.states
                .lock()
                .unwrap()
                .insert(name.to_string(), state.clone());
            Ok(())
        }
        fn delete(&self, name: &str) -> anyhow::Result<()> {
            self.manifests.lock().unwrap().remove(name);
            self.wasms.lock().unwrap().remove(name);
            self.states.lock().unwrap().remove(name);
            Ok(())
        }
        fn capability_dir(&self, name: &str) -> PathBuf {
            PathBuf::from("/tmp/conusai-mem").join(name)
        }
    }
}
