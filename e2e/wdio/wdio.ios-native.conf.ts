/**
 * WebdriverIO config — Native ConusAI Browser iOS app via Appium XCUITest.
 *
 * Tests the actual native iOS Tauri build of apps/browser-shell installed on
 * the simulator (or a real device). This is *not* Safari — Appium attaches
 * to the WKWebView inside the .app and can drive both the native chrome
 * (status bar, tab strip) and the embedded webview content.
 *
 * Build + install (once per source change):
 *   pnpm --filter browser-shell exec tauri ios build --target aarch64-sim --debug
 *   xcrun simctl install <UDID> "apps/browser-shell/src-tauri/gen/apple/build/arm64-sim/ConusAI Browser.app"
 *
 * Run tests:
 *   pnpm appium &                  # Appium server on :4723
 *   IOS_DEVICE_UDID=<udid> pnpm wdio:ios-native
 *
 * Real device: same config — set IOS_REAL_DEVICE=1 + IOS_DEVICE_UDID +
 * APPLE_TEAM_ID, build with `--target aarch64` (not -sim), and install via
 * `ios-deploy` or Xcode.
 */

import type { Options } from '@wdio/types';
import * as path from 'path';

const IOS_DEVICE_UDID = process.env.IOS_DEVICE_UDID;
const IOS_PLATFORM_VERSION = process.env.IOS_PLATFORM_VERSION ?? '18.4';
const IOS_DEVICE_NAME = process.env.IOS_DEVICE_NAME ?? 'iPhone 16 Pro';
const REAL_DEVICE = process.env.IOS_REAL_DEVICE === '1';
const BUNDLE_ID = process.env.IOS_BUNDLE_ID ?? 'com.conusai.browser';
const APP_PATH = process.env.IOS_APP_PATH ?? path.resolve(
  __dirname,
  '../../apps/browser-shell/src-tauri/gen/apple/build/arm64-sim/ConusAI Browser.app',
);

export const config: Options.Testrunner = {
  runner: 'local',
  specs: [path.join(__dirname, 'specs/ios/native.spec.ts')],
  maxInstances: 1,

  hostname: '127.0.0.1',
  port: 4723,
  path: '/',

  capabilities: [{
    platformName: 'iOS',
    'appium:automationName': 'XCUITest',
    'appium:platformVersion': IOS_PLATFORM_VERSION,
    'appium:deviceName': IOS_DEVICE_NAME,
    'appium:bundleId': BUNDLE_ID,
    // For sim: providing app path lets Appium re-install on demand.
    // For real device: ship a signed .ipa via xcodebuild and install separately.
    ...(REAL_DEVICE ? {} : { 'appium:app': APP_PATH }),
    'appium:newCommandTimeout': 240,
    'appium:autoAcceptAlerts': true,
    ...(IOS_DEVICE_UDID ? { 'appium:udid': IOS_DEVICE_UDID } : {}),
    ...(REAL_DEVICE
      ? {
          'appium:xcodeOrgId': process.env.APPLE_TEAM_ID,
          'appium:xcodeSigningId': process.env.APPLE_SIGNING_ID ?? 'iPhone Developer',
          'appium:updatedWDABundleId': process.env.WDA_BUNDLE_ID,
        }
      : {}),
  }] as WebdriverIO.Capabilities[],

  logLevel: 'warn',
  bail: 0,
  waitforTimeout: 20_000,
  connectionRetryTimeout: 120_000,
  connectionRetryCount: 3,

  framework: 'mocha',
  reporters: ['spec'],
  mochaOpts: { ui: 'bdd', timeout: 240_000 },

  autoCompileOpts: {
    autoCompile: true,
    tsNodeOpts: { transpileOnly: true, project: path.join(__dirname, 'tsconfig.json') },
  },
};
