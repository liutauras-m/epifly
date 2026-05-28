/**
 * Native auth bridge — thin JS wrapper over Rust Tauri commands.
 *
 * No token state is held in JS. All tokens live in the OS keychain
 * behind the Rust auth manager. The access token is handed out
 * per-request through auth:get_access_token and is never cached here.
 */
import { invoke } from "@tauri-apps/api/core";

/** Open the system browser to start the OIDC login flow. */
export async function startLogin(prompt?: string): Promise<void> {
  await invoke("auth_start", { prompt });
}

/**
 * Get the current access token.
 * Proactively refreshes if the token expires within 60 seconds.
 * Throws if not authenticated.
 */
export async function getAccessToken(): Promise<string> {
  return invoke<string>("auth_get_access_token");
}

/** Revoke tokens and clear the keychain session. */
export async function signOut(): Promise<void> {
  await invoke("auth_sign_out");
}
