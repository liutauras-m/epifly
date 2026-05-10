/**
 * WebdriverIO config — macOS Tauri browser-shell driven via tauri-webdriver.
 *
 * Prerequisites:
 *   1. cargo install tauri-webdriver-automation (provides `tauri-wd`)
 *   2. Build the shell with `e2e` feature in debug mode:
 *        cargo build -p browser-shell --features e2e
 *   3. Start tauri-wd in another terminal: `tauri-wd --port 4444`
 *      (or run `make wdio:macos` which orchestrates both)
 *
 * The W3C WebDriver server listens on 127.0.0.1:4444 by default.
 * tauri-wd launches the Tauri binary specified in `tauri:options.binary`.
 */

import type { Options } from '@wdio/types';
import * as path from 'path';

const REPO_ROOT = path.resolve(__dirname, '../..');
const SHELL_BINARY = path.join(REPO_ROOT, 'apps/browser-shell/src-tauri/target/debug/browser-shell');

export const config: Options.Testrunner = {
  runner: 'local',
  specs: [path.join(__dirname, 'specs/macos/**/*.spec.ts')],
  maxInstances: 1,

  hostname: '127.0.0.1',
  port: 4444,
  path: '/',

  capabilities: [{
    'tauri:options': {
      binary: SHELL_BINARY,
    },
  }] as WebdriverIO.Capabilities[],

  logLevel: 'warn',
  bail: 0,
  waitforTimeout: 10_000,
  connectionRetryTimeout: 30_000,
  connectionRetryCount: 3,

  framework: 'mocha',
  reporters: ['spec'],
  mochaOpts: { ui: 'bdd', timeout: 60_000 },

  autoCompileOpts: {
    autoCompile: true,
    tsNodeOpts: { transpileOnly: true, project: path.join(__dirname, 'tsconfig.json') },
  },
};
