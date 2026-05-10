import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './e2e',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  reporter: process.env.CI ? 'github' : 'list',
  use: {
    trace: 'on-first-retry',
  },
  projects: [
    {
      name: 'web',
      use: { ...devices['Desktop Chrome'] },
      testMatch: 'e2e/web/**/*.spec.ts',
      webServer: {
        command: 'pnpm --filter web preview',
        url: 'http://localhost:4173',
        reuseExistingServer: !process.env.CI,
      },
    },
    {
      name: 'browser-shell',
      use: { ...devices['Desktop Chrome'] },
      testMatch: 'e2e/browser-shell/**/*.spec.ts',
      // Tauri webdriver: set TAURI_WEBDRIVER_URL env when running shell e2e
      ...(process.env.TAURI_WEBDRIVER_URL
        ? {
            use: {
              connectOptions: { wsEndpoint: process.env.TAURI_WEBDRIVER_URL },
            },
          }
        : {}),
    },
  ],
});
