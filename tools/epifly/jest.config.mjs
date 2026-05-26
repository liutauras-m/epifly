/** @type {import('jest').Config} */
const config = {
  // Use ts-jest to handle TypeScript test files natively
  preset: "ts-jest/presets/js-with-ts-esm",
  testEnvironment: "node",
  extensionsToTreatAsEsm: [".ts"],
  moduleNameMapper: {
    // Map .mjs imports from dokploy/lib to the actual files via dynamic resolution
    "^(\\.{1,2}/.*)\\.mjs$": "$1.mjs",
  },
  transform: {
    "^.+\\.tsx?$": [
      "ts-jest",
      {
        useESM: true,
        tsconfig: {
          target: "ES2022",
          module: "NodeNext",
          moduleResolution: "NodeNext",
          allowJs: true,
          skipLibCheck: true,
          strict: true,
          isolatedModules: true,
          types: ["jest", "node"],
        },
      },
    ],
  },
  testMatch: [
    "<rootDir>/tests/**/*.test.ts",
  ],
  testTimeout: 15000,
  // Expose coverage for unit tests
  collectCoverageFrom: [
    "src/**/*.ts",
    "!src/cli.ts",
  ],
  // Fail fast on first test suite failure in CI
  bail: false,
};

export default config;
