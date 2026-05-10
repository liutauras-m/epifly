/**
 * WebdriverIO config — iOS Simulator (and real device) via Appium XCUITest.
 *
 * Drives mobile Safari hitting the running web app at WEB_BASE_URL (default:
 * http://localhost:4173). The same config works on a real iPhone — just set
 * IOS_DEVICE_UDID to a connected device's UDID and IOS_PLATFORM_VERSION to
 * its iOS version.
 *
 * Prerequisites:
 *   1. npm install -g appium
 *   2. appium driver install xcuitest
 *   3. xcrun simctl list devices  (find a booted iPhone simulator UDID)
 *   4. Web app running on http://localhost:4173 (`pnpm dev` or built node server)
 *   5. Real device only: WebDriverAgent built & signed with a paid Apple Dev cert
 *
 * Run:
 *   appium server --port 4723 &
 *   pnpm wdio e2e/wdio/wdio.ios.conf.ts
 */

import type { Options } from '@wdio/types';
import * as path from 'path';

const WEB_BASE_URL = process.env.WEB_BASE_URL ?? 'http://localhost:4173';
const IOS_DEVICE_UDID = process.env.IOS_DEVICE_UDID;       // optional override
const IOS_PLATFORM_VERSION = process.env.IOS_PLATFORM_VERSION ?? '18.4';
const IOS_DEVICE_NAME = process.env.IOS_DEVICE_NAME ?? 'iPhone 16 Pro';
const REAL_DEVICE = process.env.IOS_REAL_DEVICE === '1';

export const config: Options.Testrunner = {
  runner: 'local',
  specs: [path.join(__dirname, 'specs/ios/**/*.spec.ts')],
  maxInstances: 1,

  hostname: '127.0.0.1',
  port: 4723,
  path: '/',

  capabilities: [{
    platformName: 'iOS',
    'appium:automationName': 'XCUITest',
    'appium:platformVersion': IOS_PLATFORM_VERSION,
    'appium:deviceName': IOS_DEVICE_NAME,
    'appium:browserName': 'Safari',
    'appium:newCommandTimeout': 240,
    ...(IOS_DEVICE_UDID ? { 'appium:udid': IOS_DEVICE_UDID } : {}),
    // Real-device extras: a Team ID + signing identity must be configured for
    // WebDriverAgent. Provide via env so we don't bake credentials into source.
    ...(REAL_DEVICE
      ? {
          'appium:xcodeOrgId': process.env.APPLE_TEAM_ID,
          'appium:xcodeSigningId': process.env.APPLE_SIGNING_ID ?? 'iPhone Developer',
          'appium:updatedWDABundleId': process.env.WDA_BUNDLE_ID,
        }
      : {}),
  }] as WebdriverIO.Capabilities[],

  baseUrl: WEB_BASE_URL,
  logLevel: 'warn',
  bail: 0,
  waitforTimeout: 15_000,
  connectionRetryTimeout: 60_000,
  connectionRetryCount: 3,

  framework: 'mocha',
  reporters: ['spec'],
  mochaOpts: { ui: 'bdd', timeout: 120_000 },

  autoCompileOpts: {
    autoCompile: true,
    tsNodeOpts: { transpileOnly: true, project: path.join(__dirname, 'tsconfig.json') },
  },
};
