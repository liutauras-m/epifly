/**
 * Runtime-neutral platform detection.
 * Does NOT import Tauri APIs — safe to use in web, native, and feature packages.
 */
export type AppPlatform = "web" | "native";

/**
 * Returns "native" when running inside a Tauri webview, "web" otherwise.
 * Detection is based on the `__TAURI_INTERNALS__` global injected by Tauri.
 */
export function getAppPlatform(): AppPlatform {
  if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
    return "native";
  }
  return "web";
}

export function isNative(): boolean {
  return getAppPlatform() === "native";
}

export function isWeb(): boolean {
  return getAppPlatform() === "web";
}
