import { spawn, type ChildProcess } from 'node:child_process';
import { setTimeout as sleep } from 'node:timers/promises';

const WEBDRIVER_PORT = 9515;
const WEBDRIVER_URL = `http://127.0.0.1:${WEBDRIVER_PORT}`;

let tauriProc: ChildProcess | null = null;

/**
 * Spawns the Tauri app under tauri-driver (macOS WebDriver bridge).
 * Requires `tauri-driver` installed: cargo install tauri-driver
 * Returns the WebSocket endpoint for Playwright connectOptions.
 */
export async function startTauriDriver(binaryPath: string): Promise<string> {
  tauriProc = spawn('tauri-driver', ['--port', String(WEBDRIVER_PORT)], {
    env: {
      ...process.env,
      CONUSAI_E2E: '1',
      TAURI_APP_BINARY: binaryPath,
    },
    stdio: 'pipe',
  });

  tauriProc.stderr?.on('data', (d) => process.stderr.write(`[tauri-driver] ${d}`));

  // Wait for the WebDriver server to be ready
  for (let i = 0; i < 30; i++) {
    try {
      const res = await fetch(`${WEBDRIVER_URL}/status`);
      if (res.ok) break;
    } catch {
      await sleep(500);
    }
  }

  return `ws://127.0.0.1:${WEBDRIVER_PORT}`;
}

export async function stopTauriDriver() {
  if (tauriProc) {
    tauriProc.kill();
    tauriProc = null;
    await sleep(500);
  }
}

/** Resolves the debug binary path from Tauri's build output */
export function tauriBinaryPath(): string {
  const arch = process.arch === 'arm64' ? 'aarch64-apple-darwin' : 'x86_64-apple-darwin';
  return `apps/browser-shell/src-tauri/target/${arch}/debug/browser-shell`;
}
