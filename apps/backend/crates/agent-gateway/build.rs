//! Compile-time visibility into which feature flags are enabled.
//!
//! Warns during the build when the embedding stack (`local-embeddings`, a default
//! feature) is left out — such a binary's semantic router serves zero tools.
//!
//! Cargo exposes each enabled feature as `CARGO_FEATURE_<UPPER_SNAKE>`.

fn main() {
    let local_embeddings = std::env::var_os("CARGO_FEATURE_LOCAL_EMBEDDINGS").is_some();
    if !local_embeddings {
        println!("cargo:warning=agent-gateway: building WITHOUT local-embeddings");
        println!("cargo:warning=    ⚠️  Semantic router will serve ZERO tools at runtime.");
        println!(
            "cargo:warning=    Rebuild with: cargo build --features agent-gateway/local-embeddings"
        );
    }

    // Re-run only when the build.rs itself changes (don't trigger rebuilds on
    // every change in `src/`).
    println!("cargo:rerun-if-changed=build.rs");
}
