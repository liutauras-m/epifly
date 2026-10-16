//! Fail-fast environment contract.
//!
//! Services read every variable they need through one [`EnvContract`] at
//! startup, then call [`EnvContract::finish`]: a single error lists every
//! missing or invalid name, instead of failing on the first one (or worse,
//! falling back to a default nobody configured).

use std::fmt;
use std::str::FromStr;

#[derive(Debug, Default)]
pub struct EnvContract {
    missing: Vec<String>,
    invalid: Vec<String>,
}

/// Returned by [`EnvContract::finish`] when the environment does not satisfy the contract.
#[derive(Debug)]
pub struct EnvContractError {
    pub missing: Vec<String>,
    pub invalid: Vec<String>,
}

impl fmt::Display for EnvContractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "environment contract not satisfied")?;
        if !self.missing.is_empty() {
            write!(f, "; missing: {}", self.missing.join(", "))?;
        }
        if !self.invalid.is_empty() {
            write!(f, "; invalid: {}", self.invalid.join("; "))?;
        }
        Ok(())
    }
}

impl std::error::Error for EnvContractError {}

/// A variable's value when it is set and not blank.
pub fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

impl EnvContract {
    pub fn new() -> Self {
        Self::default()
    }

    /// A required variable. Records it as missing (and returns an empty string) when unset.
    pub fn required(&mut self, name: &str) -> String {
        match var(name) {
            Some(v) => v,
            None => {
                self.missing.push(name.to_string());
                String::new()
            }
        }
    }

    /// An optional variable (unset or blank → `None`).
    pub fn optional(&self, name: &str) -> Option<String> {
        var(name)
    }

    /// An optional variable parsed as `T`, `default` when unset; records a parse failure.
    pub fn parse_or<T: FromStr>(&mut self, name: &str, default: T) -> T
    where
        T::Err: fmt::Display,
    {
        match var(name) {
            None => default,
            Some(raw) => match raw.trim().parse::<T>() {
                Ok(v) => v,
                Err(e) => {
                    self.invalid(name, e);
                    default
                }
            },
        }
    }

    /// An `on`/`off` switch, `default` when unset.
    pub fn switch(&mut self, name: &str, default: bool) -> bool {
        match var(name).as_deref().map(str::trim) {
            None => default,
            Some("on") => true,
            Some("off") => false,
            Some(_) => {
                self.invalid(name, "expected `on` or `off`");
                default
            }
        }
    }

    /// Record an invalid value for `name`.
    pub fn invalid(&mut self, name: &str, reason: impl fmt::Display) {
        self.invalid.push(format!("{name}: {reason}"));
    }

    /// Ok when every required variable was present and every value valid.
    pub fn finish(self) -> Result<(), EnvContractError> {
        if self.missing.is_empty() && self.invalid.is_empty() {
            Ok(())
        } else {
            Err(EnvContractError {
                missing: self.missing,
                invalid: self.invalid,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_every_missing_name() {
        let mut c = EnvContract::new();
        let _ = c.required("COMMON_ENV_TEST_DEFINITELY_UNSET_A");
        let _ = c.required("COMMON_ENV_TEST_DEFINITELY_UNSET_B");
        let err = c.finish().unwrap_err();
        assert_eq!(err.missing.len(), 2);
        let msg = err.to_string();
        assert!(msg.contains("COMMON_ENV_TEST_DEFINITELY_UNSET_A"));
        assert!(msg.contains("COMMON_ENV_TEST_DEFINITELY_UNSET_B"));
    }

    #[test]
    fn defaults_apply_when_unset() {
        let mut c = EnvContract::new();
        assert_eq!(c.parse_or("COMMON_ENV_TEST_DEFINITELY_UNSET_C", 7u32), 7);
        assert!(c.switch("COMMON_ENV_TEST_DEFINITELY_UNSET_D", true));
        assert!(c.finish().is_ok());
    }
}
