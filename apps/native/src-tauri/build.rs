fn main() {
    // Declare the app's own Tauri commands so the ACL autogenerates
    // `allow-$command` / `deny-$command` permissions for them. These are
    // required to grant access to the commands from a remote context
    // (the iOS/dev WKWebView loads from the dev server URL, which Tauri
    // treats as "remote" — see capabilities/mobile.json `remote.urls`).
    let attributes =
        tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[
            "auth_start",
            "auth_get_access_token",
            "auth_sign_out",
        ]));
    tauri_build::try_build(attributes).expect("failed to run tauri-build");
}
