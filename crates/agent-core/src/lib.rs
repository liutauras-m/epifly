pub mod capabilities;
pub mod agent;
pub mod pipelines;
pub mod context;

pub use agent::builder::{GeneralAgent, GeneralAgentBuilder};
pub use capabilities::registry::CapabilityRegistry;
pub use capabilities::discovery::CapabilityDiscovery;
pub use pipelines::invoice::{InvoicePipeline, InvoiceData, InvoiceLineItem};
pub use context::tenant::{TenantContext, TenantClaims, PlanTier};
