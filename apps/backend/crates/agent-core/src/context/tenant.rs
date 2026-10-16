use common::types::{TenantId, UserId};
use serde::{Deserialize, Serialize};

/// Platform-level user role, from the Zitadel project roles claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UserRole {
    #[default]
    User,
    Admin,
    SuperAdmin,
}

impl std::fmt::Display for UserRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserRole::User => write!(f, "user"),
            UserRole::Admin => write!(f, "admin"),
            UserRole::SuperAdmin => write!(f, "super_admin"),
        }
    }
}

impl std::str::FromStr for UserRole {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "super_admin" => UserRole::SuperAdmin,
            "admin" => UserRole::Admin,
            _ => UserRole::User,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlanTier {
    Free,
    Pro,
    Enterprise,
}

/// Hard limits for a plan tier, resolved once per request and injected as `Extension<PlanLimits>`.
#[derive(Debug, Clone, Copy)]
pub struct PlanLimits {
    pub max_tokens: u64,
    pub max_turns: u32,
    pub rate_limit_rpm: u32,
    /// Max tool definitions sent to the LLM per turn.
    pub max_tools_per_turn: usize,
    /// Max tool invocations allowed within a single agent turn.
    pub max_invokes_per_turn: usize,
}

impl PlanTier {
    /// Resolve the hard-coded per-tier limits.
    pub fn limits(&self) -> PlanLimits {
        match self {
            PlanTier::Free => PlanLimits {
                max_tokens: 4_096,
                max_turns: 3,
                rate_limit_rpm: 10,
                max_tools_per_turn: 10,
                max_invokes_per_turn: 5,
            },
            PlanTier::Pro => PlanLimits {
                max_tokens: 16_384,
                max_turns: 8,
                rate_limit_rpm: 60,
                max_tools_per_turn: 25,
                max_invokes_per_turn: 10,
            },
            PlanTier::Enterprise => PlanLimits {
                max_tokens: 128_000,
                max_turns: 20,
                rate_limit_rpm: 600,
                max_tools_per_turn: 50,
                max_invokes_per_turn: 25,
            },
        }
    }

    pub fn collection_prefix(&self) -> &'static str {
        "capabilities"
    }

    /// Default LLM alias for agent/chat turns on this plan.
    pub fn default_alias(&self) -> &'static str {
        match self {
            PlanTier::Free => "haiku",
            PlanTier::Pro => "smart",
            PlanTier::Enterprise => "opus",
        }
    }

    /// LLM aliases a client may request for agent/chat turns on this plan.
    /// The model is always chosen server-side from this list.
    pub fn allowed_aliases(&self) -> &'static [&'static str] {
        match self {
            PlanTier::Free => &["haiku", "fast", "cheap"],
            PlanTier::Pro => &["haiku", "fast", "cheap", "smart"],
            PlanTier::Enterprise => &["haiku", "fast", "cheap", "smart", "opus"],
        }
    }
}

impl std::fmt::Display for PlanTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlanTier::Free => write!(f, "free"),
            PlanTier::Pro => write!(f, "pro"),
            PlanTier::Enterprise => write!(f, "enterprise"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TenantContext {
    pub tenant_id: TenantId,
    pub user_id: Option<UserId>,
    pub plan: PlanTier,
    pub role: UserRole,
}

impl TenantContext {
    pub fn new(
        tenant_id: impl Into<TenantId>,
        user_id: Option<impl Into<UserId>>,
        plan: PlanTier,
    ) -> Self {
        Self {
            tenant_id: tenant_id.into(),
            user_id: user_id.map(Into::into),
            plan,
            role: UserRole::User,
        }
    }

    /// Storage namespace prefix for this tenant.
    pub fn tenant_namespace(&self, kind: &str) -> String {
        format!("{}_{}", kind, &*self.tenant_id)
    }

    /// Default system prompt injected into every agent turn.
    pub fn system_prompt(&self) -> String {
        format!(
            "You are a helpful AI assistant for tenant {}. Plan tier: {}.",
            &*self.tenant_id, self.plan
        )
    }

    /// Tracing fields to attach to every span.
    pub fn span_fields(&self) -> Vec<(&'static str, String)> {
        let mut fields = vec![("tenant_id", self.tenant_id.to_string())];
        if let Some(uid) = &self.user_id {
            fields.push(("user_id", uid.to_string()));
        }
        fields.push(("plan", self.plan.to_string()));
        fields
    }
}
