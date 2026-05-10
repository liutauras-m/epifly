import { defineConfig, devices } from '@playwright/test';

const webServerUrl = 'http://localhost:4173';

export default defineConfig({
  testDir: './e2e',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  reporter: process.env.CI ? 'github' : 'list',
  use: {
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
  },

  // Single web server shared by web + ios projects.
  // adapter-node build is the SSR server; run from repo root.
  webServer: {
    command: `PORT=4173 HOST=127.0.0.1 ORIGIN=${webServerUrl} node apps/web/build/index.js`,
    url: webServerUrl,
    reuseExistingServer: !process.env.CI,
    timeout: 30_000,
    stdout: 'pipe',
    stderr: 'pipe',
  },

  projects: [
    // ── Web (Desktop Chrome) ─────────────────────────────────────────────────
    {
      name: 'web',
      use: { ...devices['Desktop Chrome'], baseURL: webServerUrl },
      testMatch: 'e2e/web/**/*.spec.ts',
    },

    // ── iOS Mobile Safari emulation ──────────────────────────────────────────
    // Covers responsive CSS, touch targets, mobile layout — no Tauri build needed.
    {
      name: 'ios-mobile-web',
      use: { ...devices['iPhone 15'], baseURL: webServerUrl },
      testMatch: 'e2e/ios/**/*.spec.ts',
    },

    // ── macOS Browser Shell (Tauri via tauri-driver WebDriver bridge) ─────────
    // Install: cargo install tauri-driver
    // Run: TAURI_WEBDRIVER_URL=$(make start-tauri-driver) pnpm e2e:shell
    // Tests self-skip when TAURI_WEBDRIVER_URL is unset.
    {
      name: 'shell-macos',
      use: process.env.TAURI_WEBDRIVER_URL
        ? { connectOptions: { wsEndpoint: process.env.TAURI_WEBDRIVER_URL } }
        : { ...devices['Desktop Chrome'] },
      testMatch: 'e2e/shell-macos/**/*.spec.ts',
    },
  ],
});
