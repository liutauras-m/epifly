//! UI module — Foundry chat interface.
//!
//! Server-side rendered (Askama) + thin client (vanilla JS for streaming,
//! Alpine-style attributes via direct event delegation). Self-contained:
//! no SPA build step, no external API calls from the page.

pub mod handlers;
pub mod routes;
pub mod session;
pub mod view;

pub use routes::ui_router;
