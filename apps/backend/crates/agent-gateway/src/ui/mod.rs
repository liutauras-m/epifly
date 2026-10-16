//! UI API module — authenticated endpoints for streaming, file upload and download.
//! HTML rendering is handled by the SvelteKit frontend (apps/web).

pub mod handlers;
pub mod routes;

pub use routes::ui_router;
