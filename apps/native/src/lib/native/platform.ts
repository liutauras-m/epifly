/**
 * Runtime platform detection — runs inside the Tauri webview only.
 * Import only from apps/native, never from packages/ui or packages/features.
 */
export type Platform = "desktop" | "ios" | "android";

export function getPlatform(): Platform {
  if (typeof window === "undefined") return "desktop";
  const ua = navigator.userAgent;
  if (/iPhone|iPad|iPod/.test(ua)) return "ios";
  if (/Android/.test(ua)) return "android";
  return "desktop";
}

export function isMobile(): boolean {
  const p = getPlatform();
  return p === "ios" || p === "android";
}

export function isDesktop(): boolean {
  return getPlatform() === "desktop";
}
