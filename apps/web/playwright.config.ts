import { defineConfig, devices } from "@playwright/test";
import { randomBytes } from "node:crypto";

// The web server needs the full BFF environment. Sign-in goes to the mock IdP
// (e2e/fixtures/mock-oidc.mjs at the repo root); the gateway is not running,
// so SSR gateway calls fail soft. The session key is random per run.
const ORIGIN = process.env.BASE_URL ?? "http://localhost:4173";
const MOCK_OIDC_PORT = 4180;
const serverEnv = [
  `PORT=4173`,
  `ORIGIN=${ORIGIN}`,
  `GATEWAY_URL=http://127.0.0.1:4199`,
  `ZITADEL_ISSUER=http://127.0.0.1:${MOCK_OIDC_PORT}`,
  `ZITADEL_WEB_CLIENT_ID=e2e-web`,
  `AUTH_SESSION_KEY=${randomBytes(32).toString("base64")}`,
].join(" ");

export default defineConfig({
  testDir: "./e2e",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: process.env.CI ? "github" : "list",
  use: {
    baseURL: ORIGIN,
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
  webServer: [
    {
      command: `MOCK_OIDC_PORT=${MOCK_OIDC_PORT} node ../../e2e/fixtures/mock-oidc.mjs`,
      url: `http://127.0.0.1:${MOCK_OIDC_PORT}/healthz`,
      reuseExistingServer: !process.env.CI,
      timeout: 10_000,
    },
    {
      command: `${serverEnv} node build`,
      url: `${ORIGIN}/healthz`,
      reuseExistingServer: !process.env.CI,
      timeout: 30_000,
    },
  ],
});
