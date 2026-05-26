/**
 * Safe-area CSS custom properties injected at runtime from Tauri/native insets.
 * Values fall back to env() in CSS; this module allows programmatic override
 * if a native plugin exposes exact inset values.
 */
export function applySafeAreaInsets(insets: {
  top?: number;
  bottom?: number;
  left?: number;
  right?: number;
}): void {
  const root = document.documentElement;
  if (insets.top !== undefined) root.style.setProperty("--safe-top", `${insets.top}px`);
  if (insets.bottom !== undefined) root.style.setProperty("--safe-bottom", `${insets.bottom}px`);
  if (insets.left !== undefined) root.style.setProperty("--safe-left", `${insets.left}px`);
  if (insets.right !== undefined) root.style.setProperty("--safe-right", `${insets.right}px`);
}
