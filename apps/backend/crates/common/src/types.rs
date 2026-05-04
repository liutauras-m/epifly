/// Typed ID newtypes — compile-time guarantees that the right ID kind is
/// passed to the right function.  All are `serde(transparent)` so the wire
/// format is unchanged (a plain ULID or string).
use serde::{Deserialize, Serialize};
use ulid::Ulid;

// ── ULID-backed newtypes ──────────────────────────────────────────────────────

macro_rules! ulid_newtype {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub Ulid);

        impl $name {
            pub fn new() -> Self {
                Self(Ulid::new())
            }

            pub fn inner(&self) -> &Ulid {
                &self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }

        impl std::str::FromStr for $name {
            type Err = ulid::DecodeError;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self(s.parse()?))
            }
        }

        impl From<Ulid> for $name {
            fn from(u: Ulid) -> Self {
                Self(u)
            }
        }

        impl From<$name> for Ulid {
            fn from(n: $name) -> Ulid {
                n.0
            }
        }
    };
}

ulid_newtype!(ThreadId);
ulid_newtype!(NodeId);

// ── String-backed newtypes (tenant/user IDs are arbitrary strings) ────────────

macro_rules! string_newtype {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            pub fn new(s: impl Into<String>) -> Self {
                Self(s.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl std::ops::Deref for $name {
            type Target = str;
            fn deref(&self) -> &str {
                &self.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self(s)
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self(s.to_owned())
            }
        }
    };
}

string_newtype!(TenantId);
string_newtype!(UserId);

/// Opaque tool identifier (string slug from capability.toml).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ToolId(pub String);

impl ToolId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
}

impl std::fmt::Display for ToolId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thread_id_roundtrip() {
        let id = ThreadId::new();
        let s = id.to_string();
        let back: ThreadId = s.parse().unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn tenant_id_deref() {
        let t = TenantId::new("acme");
        let s: &str = &t;
        assert_eq!(s, "acme");
    }

    #[test]
    fn thread_id_serde_transparent() {
        let id = ThreadId::new();
        let json = serde_json::to_string(&id).unwrap();
        // Should be a plain quoted ULID string, not a wrapped object
        assert!(json.starts_with('"'));
        let back: ThreadId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }
}
