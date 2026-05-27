//! Workspace delete-plan types.
//!
//! The canonical type is `common::memory::store::DeletePlanNode` (lives in `common` to avoid
//! a circular dep). This module re-exports it under the names the plan spec uses, so external
//! callers in `agent-gateway` can use `agent_core::DeletePlan` / `agent_core::DeletePlanNode`
//! without reaching into `common` directly.

pub use common::memory::store::DeletePlanNode;
