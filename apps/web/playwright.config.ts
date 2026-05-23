import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: process.env.CI ? "github" : "list",
  use: {
    baseURL: process.env.BASE_URL ?? "http://localhost:4173",
    trace: "on-first-retry",
  },
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
    {
      // Visual regression project — run with `just visual` (Docker) or
      // `pnpm --filter web exec playwright test --project=visual`.
      // Snapshots are stored under e2e/__screenshots__/ and committed.
      name: "visual",
      testMatch: "**/visual/**/*.spec.ts",
      use: {
        ...devices["Desktop Chrome"],
        // Screenshots stored per project/theme/viewport, set in spec via
        // the toHaveScreenshot() filename argument.
      },
      snapshotDir: "./e2e/__screenshots__",
      snapshotPathTemplate: "{snapshotDir}/{testFilePath}/{arg}{ext}",
    },
  ],
  webServer: {
    command: "pnpm preview",
    url: "http://localhost:4173",
    reuseExistingServer: !process.env.CI,
    timeout: 30_000,
  },
});
