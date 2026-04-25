use agent_core::{CapabilityDiscovery, CapabilityRegistry};
use std::sync::Mutex;
use crate::mw::RateLimiter;

pub struct AppState {
    pub registry: Mutex<CapabilityRegistry>,
    pub rate_limiter: RateLimiter,
}

impl AppState {
    pub fn from_env() -> common::error::Result<Self> {
        let discovery = CapabilityDiscovery::from_env();
        let registry = discovery.discover()?;
        Ok(Self {
            registry: Mutex::new(registry),
            rate_limiter: RateLimiter::new(),
        })
    }
}
