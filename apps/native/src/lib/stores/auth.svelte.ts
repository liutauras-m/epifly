/**
 * Reactive auth state for the native app.
 *
 * Wraps the Rust Tauri auth commands and emits events so the root layout
 * can show the login screen vs the app shell.
 *
 * State machine:
 *   checking → authenticated | unauthenticated
 *   login_pending (system browser open)
 *   unauthenticated → login_pending → authenticated
 *   authenticated → unauthenticated (sign-out)
 */
import { listen } from "@tauri-apps/api/event";
import { startLogin, signOut, getAccessToken } from "$lib/native/auth.js";

type AuthStatus = "checking" | "authenticated" | "unauthenticated" | "login_pending";

/** True only inside a Tauri WKWebView where the native bridge is injected. */
const isTauri = (): boolean =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

class NativeAuthStore {
  status: AuthStatus = $state("checking");
  error: string | null = $state(null);

  constructor() {
    if (isTauri()) {
      // Wire up Tauri event listeners once at store creation.
      void this.#setupListeners();
    }
    void this.#checkInitialState();
  }

  async #setupListeners(): Promise<void> {
    // auth:signed_in fires after the deep-link callback completes token exchange
    await listen<{ org_id: string }>("auth:signed_in", () => {
      this.status = "authenticated";
      this.error = null;
    });

    // auth:signed_out fires after sign_out clears the keychain
    await listen("auth:signed_out", () => {
      this.status = "unauthenticated";
      this.error = null;
    });

    // auth:error fires if code exchange fails
    await listen<string>("auth:error", (event) => {
      this.status = "unauthenticated";
      this.error = event.payload ?? "Authentication failed";
    });
  }

  async #checkInitialState(): Promise<void> {
    if (!isTauri()) {
      // Non-Tauri browser context (dev/preview) — treat as unauthenticated
      this.status = "unauthenticated";
      return;
    }
    try {
      await getAccessToken();
      this.status = "authenticated";
    } catch {
      // Not authenticated — show login
      this.status = "unauthenticated";
    }
  }

  async login(): Promise<void> {
    if (!isTauri()) {
      // Non-Tauri browser context — auth is not available
      this.error = "Sign in requires the Epifly app.";
      return;
    }
    this.error = null;
    this.status = "login_pending";
    try {
      await startLogin();
      // Status will transition to "authenticated" via the auth:signed_in event
    } catch (e) {
      // Sanitize technical errors — never show raw JS stack traces to the user
      const raw = e instanceof Error ? e.message : String(e);
      this.error = raw.includes("ZITADEL") || raw.includes("not set")
        ? "Authentication is not configured. Contact support."
        : "Could not open the sign-in page. Please try again.";
      this.status = "unauthenticated";
    }
  }

  async logout(): Promise<void> {
    try {
      await signOut();
      // Status will transition to "unauthenticated" via the auth:signed_out event
    } catch {
      // Force unauthenticated even on error (keychain already cleared)
      this.status = "unauthenticated";
    }
  }
}

// Singleton — shared across the whole app
export const auth = new NativeAuthStore();
