fn main() {
    // Public build-time configuration baked into the binary (see src/auth/mod.rs).
    for var in ["VITE_ZITADEL_ISSUER", "VITE_ZITADEL_NATIVE_CLIENT_ID", "VITE_GATEWAY_URL"] {
        println!("cargo:rerun-if-env-changed={var}");
    }
    tauri_build::build();
}
