//! Compile-time visibility into which feature flags are enabled.
//!
//! Per Phase 1.4.2 of `docs/plan.md`. Emits a single `cargo:warning=...`
//! during build so engineers can see at a glance whether the binary they
//! just produced includes the embedding stack — instead of finding out
//! at first chat that the router serves zero tools.
//!
//! Cargo exposes each enabled feature as `CARGO_FEATURE_<UPPER_SNAKE>`.

fn main() {
    let local_embeddings = std::env::var_os("CARGO_FEATURE_LOCAL_EMBEDDINGS").is_some();
    println!(
        "cargo:warning=agent-gateway: building with local-embeddings = {local_embeddings}"
    );
    if !local_embeddings {
        println!(
            "cargo:warning=    ⚠️  Semantic router will serve ZERO tools at runtime."
        );
        println!(
            "cargo:warning=    Rebuild with: cargo build --features agent-gateway/local-embeddings"
        );
    }

    // Re-run only when the build.rs itself changes (don't trigger rebuilds on
    // every change in `src/`).
    println!("cargo:rerun-if-changed=build.rs");
}
