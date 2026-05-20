import type { Options } from '@wdio/types';
import * as path from 'path';

const UDID = '64897BF0-B403-4104-BBFE-0250990F11A5';

export const config: Options.Testrunner = {
  runner: 'local',
  specs: [path.join(__dirname, 'specs/ios/invoice_test.spec.ts')],
  maxInstances: 1,
  hostname: '127.0.0.1',
  port: 4723,
  path: '/',
  capabilities: [{
    platformName: 'iOS',
    'appium:automationName': 'XCUITest',
    'appium:platformVersion': '18.4',
    'appium:deviceName': 'iPhone 16 Pro',
    'appium:bundleId': 'com.conusai.browser',
    'appium:udid': UDID,
    'appium:noReset': true,
    'appium:newCommandTimeout': 300,
    'appium:autoAcceptAlerts': true,
    'appium:enableWebviewDetailsCollection': true,
    'appium:webviewConnectTimeout': 30000,
    'appium:includeSafariInWebviews': true,
    'appium:additionalWebviewBundleIds': ['process-ConusAI Browser', 'ConusAI Browser'],
  }] as WebdriverIO.Capabilities[],
  logLevel: 'warn',
  bail: 0,
  waitforTimeout: 30_000,
  connectionRetryTimeout: 120_000,
  connectionRetryCount: 3,
  framework: 'mocha',
  reporters: ['spec'],
  mochaOpts: { ui: 'bdd', timeout: 300_000 },
  autoCompileOpts: {
    autoCompile: true,
    tsNodeOpts: { transpileOnly: true, project: path.join(__dirname, 'tsconfig.json') },
  },
};
