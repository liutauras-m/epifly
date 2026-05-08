//! UI API module — session-authenticated endpoints for streaming, file upload, and invoice extraction.
//! HTML rendering is handled by the SvelteKit frontend (apps/web).

pub mod handlers;
pub mod routes;
pub mod session;

pub use routes::ui_router;
