//! Plan limit clamping tests.
//!
//! Verifies that `PlanTier::limits()` returns correct per-tier caps.
//! RouterQuotaLayer per-plan override is tested inline in `mw/router_quota.rs`.

use agent_core::context::tenant::PlanTier;

// ── PlanTier::limits() field assertions ───────────────────────────────────────

#[test]
fn free_tier_limits() {
    let l = PlanTier::Free.limits();
    assert_eq!(l.max_tokens, 4_096);
    assert_eq!(l.max_turns, 3);
    assert_eq!(l.rate_limit_rpm, 10);
    assert_eq!(l.max_tools_per_turn, 10);
    assert_eq!(l.max_invokes_per_turn, 5);
}

#[test]
fn pro_tier_limits() {
    let l = PlanTier::Pro.limits();
    assert_eq!(l.max_tokens, 16_384);
    assert_eq!(l.max_turns, 8);
    assert_eq!(l.rate_limit_rpm, 60);
    assert_eq!(l.max_tools_per_turn, 25);
    assert_eq!(l.max_invokes_per_turn, 10);
}

#[test]
fn enterprise_tier_limits() {
    let l = PlanTier::Enterprise.limits();
    assert_eq!(l.max_tokens, 128_000);
    assert_eq!(l.max_turns, 20);
    assert_eq!(l.rate_limit_rpm, 600);
    assert_eq!(l.max_tools_per_turn, 50);
    assert_eq!(l.max_invokes_per_turn, 25);
}

#[test]
fn tier_limits_are_strictly_ordered() {
    let free = PlanTier::Free.limits();
    let pro = PlanTier::Pro.limits();
    let ent = PlanTier::Enterprise.limits();

    assert!(free.max_tokens < pro.max_tokens && pro.max_tokens < ent.max_tokens);
    assert!(free.max_turns < pro.max_turns && pro.max_turns < ent.max_turns);
    assert!(free.rate_limit_rpm < pro.rate_limit_rpm && pro.rate_limit_rpm < ent.rate_limit_rpm);
    assert!(
        free.max_tools_per_turn < pro.max_tools_per_turn
            && pro.max_tools_per_turn < ent.max_tools_per_turn
    );
    assert!(
        free.max_invokes_per_turn < pro.max_invokes_per_turn
            && pro.max_invokes_per_turn < ent.max_invokes_per_turn
    );
}
