//! Re-exports from `crate::auth` — kept for `/ui/*` handler import compatibility.
//! All auth logic lives in `crate::auth`.
pub use crate::auth::{COOKIE_NAME, SessionUser, verify};
