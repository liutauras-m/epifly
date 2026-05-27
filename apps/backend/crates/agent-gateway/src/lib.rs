//! agent-gateway library crate — re-exports all modules so integration tests can access them.
//!
//! The binary entry point is `main.rs` which calls `run()`.  All logic lives in these
//! modules; `main.rs` is intentionally thin.
pub mod agent;
pub mod auth;
pub mod capabilities;
pub mod metrics;
pub mod mw;
pub mod routes;
pub mod state;
pub mod ui;
