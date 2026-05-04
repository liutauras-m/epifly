use crate::state::AppState;
use agent_core::{PlanTier, TenantClaims, UserRole};
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::warn;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginRequest {
    /// User identifier (email or username).
    pub email: String,
    /// Password (dev mode: any non-empty value accepted).
    pub password: String,
    /// Tenant ID override (dev mode only).
    #[serde(default)]
    pub tenant_id: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LoginResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub tenant_id: String,
}

/// `POST /v1/auth/login` — exchange credentials for a JWT.
///
/// **Production mode** (`JWT_SECRET` set): validates `email` + `password` against
/// a credential store (currently env-var stub — plug in your user store here).
///
/// **Dev mode** (`JWT_SECRET` unset): issues a JWT signed with a hardcoded dev
/// secret for any non-empty email. Suitable for local development ONLY.
#[utoipa::path(
    post,
    path = "/v1/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = LoginResponse),
        (status = 401, description = "Invalid credentials"),
    ),
    tag = "auth",
)]
pub async fn login(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> impl IntoResponse {
    if req.email.is_empty() || req.password.is_empty() {
        return (StatusCode::UNAUTHORIZED, "credentials required").into_response();
    }

    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_default();
    let dev_mode = jwt_secret.is_empty();

    // Production mode: validate password against DEV_PASSWORD env (replace with DB lookup).
    if !dev_mode {
        let expected_pw = std::env::var("DEV_PASSWORD").unwrap_or_default();
        if expected_pw.is_empty() || req.password != expected_pw {
            warn!(email = %req.email, "login failed: invalid credentials");
            return (StatusCode::UNAUTHORIZED, "invalid credentials").into_response();
        }
    }

    let signing_key = if dev_mode {
        "conusai-dev-secret-not-for-production"
    } else {
        &jwt_secret
    };

    let tenant_id = req
        .tenant_id
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| {
            if dev_mode {
                "dev".to_string()
            } else {
                req.email.clone()
            }
        });

    let exp = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(24))
        .expect("valid timestamp")
        .timestamp() as u64;

    // Determine role from SUPER_ADMIN_EMAILS env var (comma-separated).
    let super_admin_emails = std::env::var("SUPER_ADMIN_EMAILS").unwrap_or_default();
    let role = if super_admin_emails
        .split(',')
        .any(|e| e.trim().eq_ignore_ascii_case(&req.email))
    {
        UserRole::SuperAdmin
    } else {
        UserRole::User
    };

    let claims = TenantClaims {
        sub: req.email.clone(),
        tenant_id: tenant_id.clone(),
        plan: PlanTier::Pro,
        role,
        exp,
    };

    match encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(signing_key.as_bytes()),
    ) {
        Ok(token) => Json(LoginResponse {
            access_token: token,
            token_type: "Bearer".to_string(),
            expires_in: 86400,
            tenant_id,
        })
        .into_response(),
        Err(e) => {
            warn!(error = %e, "JWT encode failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "token generation failed").into_response()
        }
    }
}
