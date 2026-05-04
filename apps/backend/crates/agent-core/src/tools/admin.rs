//! High-level admin operations for capabilities: CRUD, toggle, reload, test.
//!
//! `CapabilityAdmin` is the single place that coordinates:
//! - `RegisteredToolStore`  — filesystem persistence
//! - `ToolRegistry`         — in-memory state
//! - `RegisteredToolValidator` — input validation
//! - `AuditStore`           — event recording

use super::card::CapabilityCard;
use super::manifest::ToolManifest;
use super::store::{FilesystemStore, RegisteredToolState, RegisteredToolStore};
use super::validator::RegisteredToolValidator;
use super::registry::ToolRegistry;
use crate::context::tenant::TenantContext;
use common::audit::{AuditEvent, AuditStore};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

// ── Limits ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AdminLimits {
    pub max_capabilities: usize,
    pub max_manifest_bytes: usize,
    pub max_wasm_bytes: usize,
}

impl Default for AdminLimits {
    fn default() -> Self {
        Self {
            max_capabilities: 64,
            max_manifest_bytes: 65_536,
            max_wasm_bytes: 8_388_608, // 8 MiB
        }
    }
}

impl AdminLimits {
    /// Load limits from environment variables with built-in defaults.
    pub fn from_env() -> Self {
        fn env_usize(key: &str, default: usize) -> usize {
            std::env::var(key)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default)
        }
        Self {
            max_capabilities: env_usize("CONUSAI_MAX_CAPABILITIES", 64),
            max_manifest_bytes: env_usize("CONUSAI_MAX_MANIFEST_BYTES", 65_536),
            max_wasm_bytes: env_usize("CONUSAI_MAX_WASM_BYTES", 8_388_608),
        }
    }
}

// ── DTOs ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilitySummary {
    pub name: String,
    pub version: String,
    pub description: String,
    pub kind: String,
    pub enabled: bool,
    pub tags: Vec<String>,
    pub last_error: Option<String>,
    pub registered_at: String,
    pub updated_at: String,
}

impl From<&CapabilityCard> for CapabilitySummary {
    fn from(c: &CapabilityCard) -> Self {
        use std::time::UNIX_EPOCH;
        let fmt = |t: std::time::SystemTime| {
            let secs = t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
            chrono::DateTime::<Utc>::from_timestamp(secs as i64, 0)
                .map(|d| d.to_rfc3339())
                .unwrap_or_default()
        };
        Self {
            name: c.manifest.name.clone(),
            version: c.manifest.version.clone(),
            description: c.manifest.description.clone(),
            kind: format!("{:?}", c.manifest.kind).to_ascii_lowercase(),
            enabled: c.enabled,
            tags: c.manifest.tags.clone(),
            last_error: c.last_error.clone(),
            registered_at: fmt(c.registered_at),
            updated_at: fmt(c.updated_at),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateCapabilityRequest {
    pub manifest_toml: String,
    /// Optional WASM bytes (base64-encoded in JSON transport, raw bytes in multipart).
    #[serde(default)]
    pub wasm_bytes: Option<Vec<u8>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateCapabilityRequest {
    pub manifest_toml: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TestInvokeRequest {
    pub tool_name: String,
    pub input: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TestInvokeResponse {
    pub output: serde_json::Value,
    pub duration_ms: u64,
}

// ── Admin service ─────────────────────────────────────────────────────────────

pub struct CapabilityAdmin {
    store: Arc<dyn RegisteredToolStore>,
    registry: Arc<Mutex<ToolRegistry>>,
    audit: Arc<dyn AuditStore>,
    limits: AdminLimits,
}

impl CapabilityAdmin {
    pub fn new(
        store: Arc<dyn RegisteredToolStore>,
        registry: Arc<Mutex<ToolRegistry>>,
        audit: Arc<dyn AuditStore>,
    ) -> Self {
        Self {
            store,
            registry,
            audit,
            limits: AdminLimits::default(),
        }
    }

    pub fn with_limits(mut self, limits: AdminLimits) -> Self {
        self.limits = limits;
        self
    }

    // ── Queries ───────────────────────────────────────────────────────────────

    pub fn list(&self) -> Vec<CapabilitySummary> {
        let reg = self.registry.lock().unwrap();
        reg.all().map(CapabilitySummary::from).collect()
    }

    pub fn get(&self, name: &str) -> Option<CapabilitySummary> {
        let reg = self.registry.lock().unwrap();
        reg.get(name).map(CapabilitySummary::from)
    }

    pub fn get_manifest_toml(&self, name: &str) -> anyhow::Result<String> {
        self.store.read_manifest(name)
    }

    // ── Mutations ─────────────────────────────────────────────────────────────

    pub fn create(
        &self,
        req: CreateCapabilityRequest,
        actor: &TenantContext,
    ) -> anyhow::Result<CapabilitySummary> {
        // Check capacity limit.
        let current_count = self.registry.lock().unwrap().len();
        if current_count >= self.limits.max_capabilities {
            anyhow::bail!("max_capabilities limit ({}) reached", self.limits.max_capabilities);
        }

        // Validate manifest size.
        let size_report = RegisteredToolValidator::validate_manifest_size(
            &req.manifest_toml,
            self.limits.max_manifest_bytes,
        );
        if !size_report.ok() {
            anyhow::bail!("{}", size_report.errors[0]);
        }

        // Validate manifest contents.
        let report = RegisteredToolValidator::validate_manifest(&req.manifest_toml);
        if !report.ok() {
            anyhow::bail!("{}", report.errors[0]);
        }

        // Parse manifest to get the name.
        let manifest = ToolManifest::from_toml(&req.manifest_toml)?;
        let name = manifest.name.clone();

        // Validate WASM if provided.
        if let Some(wasm) = &req.wasm_bytes {
            let wasm_report = RegisteredToolValidator::validate_wasm(wasm, self.limits.max_wasm_bytes);
            if !wasm_report.ok() {
                anyhow::bail!("{}", wasm_report.errors[0]);
            }
        }

        // Write to disk.
        self.store.write_manifest(&name, &req.manifest_toml)?;
        if let Some(wasm) = &req.wasm_bytes {
            self.store.write_wasm(&name, wasm)?;
        }
        let state = RegisteredToolState {
            enabled: true,
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
        };
        self.store.write_state(&name, &state)?;

        // Hot-load into registry.
        let cap_dir = self.store.capability_dir(&name);
        self.registry.lock().unwrap().reload_capability(&cap_dir)?;

        let summary = self.get(&name).expect("just registered");

        // Audit.
        self.emit_audit(actor, "capability.create", &name, "ok");
        Ok(summary)
    }

    pub fn update(
        &self,
        name: &str,
        req: UpdateCapabilityRequest,
        actor: &TenantContext,
    ) -> anyhow::Result<CapabilitySummary> {
        let size_report = RegisteredToolValidator::validate_manifest_size(
            &req.manifest_toml,
            self.limits.max_manifest_bytes,
        );
        if !size_report.ok() {
            anyhow::bail!("{}", size_report.errors[0]);
        }

        let report = RegisteredToolValidator::validate_manifest(&req.manifest_toml);
        if !report.ok() {
            anyhow::bail!("{}", report.errors[0]);
        }

        // Ensure name in manifest matches URL param.
        let manifest = ToolManifest::from_toml(&req.manifest_toml)?;
        if manifest.name != name {
            anyhow::bail!("manifest name '{}' does not match capability name '{name}'", manifest.name);
        }

        self.store.write_manifest(name, &req.manifest_toml)?;

        // Refresh state timestamp.
        let prev_state = self.store.read_state(name)?.unwrap_or_else(|| RegisteredToolState {
            enabled: true,
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
        });
        self.store.write_state(name, &RegisteredToolState {
            updated_at: Utc::now().to_rfc3339(),
            ..prev_state
        })?;

        let cap_dir = self.store.capability_dir(name);
        self.registry.lock().unwrap().reload_capability(&cap_dir)?;

        let summary = self.get(name).expect("just reloaded");
        self.emit_audit(actor, "capability.update", name, "ok");
        Ok(summary)
    }

    pub fn set_enabled(
        &self,
        name: &str,
        enabled: bool,
        actor: &TenantContext,
    ) -> anyhow::Result<CapabilitySummary> {
        {
            let mut reg = self.registry.lock().unwrap();
            if !reg.set_enabled(name, enabled) {
                anyhow::bail!("capability not found: {name}");
            }
        }

        // Persist state.
        let prev_state = self.store.read_state(name)?.unwrap_or_else(|| RegisteredToolState {
            enabled,
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
        });
        self.store.write_state(name, &RegisteredToolState {
            enabled,
            updated_at: Utc::now().to_rfc3339(),
            ..prev_state
        })?;

        let action = if enabled { "capability.enable" } else { "capability.disable" };
        self.emit_audit(actor, action, name, "ok");
        Ok(self.get(name).expect("just updated"))
    }

    pub fn delete(&self, name: &str, actor: &TenantContext) -> anyhow::Result<()> {
        self.registry.lock().unwrap().unregister(name);
        self.store.delete(name)?;
        self.emit_audit(actor, "capability.delete", name, "ok");
        Ok(())
    }

    pub fn reload(&self, name: &str, actor: &TenantContext) -> anyhow::Result<CapabilitySummary> {
        let cap_dir = self.store.capability_dir(name);
        self.registry.lock().unwrap().reload_capability(&cap_dir)?;
        let summary = self.get(name).expect("just reloaded");
        self.emit_audit(actor, "capability.reload", name, "ok");
        Ok(summary)
    }

    pub fn reload_all(&self, actor: &TenantContext) -> anyhow::Result<usize> {
        let names = self.store.list()?;
        let mut count = 0;
        for name in &names {
            let cap_dir = self.store.capability_dir(name);
            match self.registry.lock().unwrap().reload_capability(&cap_dir) {
                Ok(()) => count += 1,
                Err(e) => tracing::warn!(name=%name, error=%e, "reload_all: reload failed"),
            }
        }
        self.emit_audit(actor, "capability.reload_all", "*", "ok");
        Ok(count)
    }

    pub async fn test_invoke(
        &self,
        req: TestInvokeRequest,
        tenant: TenantContext,
    ) -> anyhow::Result<TestInvokeResponse> {
        let provider = {
            let reg = self.registry.lock().unwrap();
            reg.get_provider(&req.tool_name)
                .ok_or_else(|| anyhow::anyhow!("capability '{}' not found or no provider", req.tool_name))?
        };

        let start = std::time::Instant::now();
        let output = provider
            .invoke(&req.tool_name, &req.input, Some(&tenant))
            .await?;
        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(TestInvokeResponse { output, duration_ms })
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn emit_audit(&self, actor: &TenantContext, action: &str, tool: &str, status: &str) {
        let event = AuditEvent::new(actor.tenant_id.as_str(), action)
            .with_tool(tool)
            .with_status(status);
        let audit = Arc::clone(&self.audit);
        tokio::spawn(async move {
            let _ = audit.append(event).await;
        });
    }
}

/// Build a `CapabilityAdmin` from `AppState`-like components.
pub fn build_admin(
    registry: Arc<Mutex<ToolRegistry>>,
    audit: Arc<dyn AuditStore>,
) -> CapabilityAdmin {
    let store = Arc::new(FilesystemStore::from_env());
    CapabilityAdmin::new(store, registry, audit)
        .with_limits(AdminLimits::from_env())
}
