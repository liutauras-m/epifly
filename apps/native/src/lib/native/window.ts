/**
 * Tauri window utilities — import only inside apps/native.
 * Wraps window management so components stay Tauri-free.
 */
export async function setWindowTitle(title: string): Promise<void> {
  // Dynamically import Tauri so the module doesn't break in non-Tauri builds.
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await getCurrentWindow().setTitle(title);
}

export async function minimizeWindow(): Promise<void> {
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await getCurrentWindow().minimize();
}
