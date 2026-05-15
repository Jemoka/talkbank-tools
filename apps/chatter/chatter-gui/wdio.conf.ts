/**
 * WebdriverIO configuration for Chatter Desktop E2E tests.
 *
 * Uses tauri-driver as the WebDriver backend to drive the real Tauri app.
 *
 * Prerequisites:
 *   cargo install tauri-driver
 *   cargo tauri build --debug    (or use the dev binary)
 *
 * Run:
 *   npx wdio run wdio.conf.ts
 */

import { join } from "path";

const isCI = !!process.env.CI;

// Path to the built Tauri app binary
const appBinary = join(
  __dirname,
  "..",
  "target",
  "debug",
  "chatter-gui",
);

export const config: WebdriverIO.Config = {
  runner: "local",
  specs: ["./tests/e2e/**/*.spec.ts"],

  maxInstances: 1,
  capabilities: [
    {
      // Tauri uses the native WebDriver endpoint via tauri-driver
      "tauri:options": {
        application: appBinary,
      },
    } as never,
  ],

  logLevel: isCI ? "warn" : "info",
  bail: 0,
  waitforTimeout: 10000,
  connectionRetryTimeout: 120000,
  connectionRetryCount: 3,

  // tauri-driver listens on port 4444 by default
  port: 4444,
  hostname: "localhost",

  services: [],
  framework: "mocha",
  reporters: ["spec"],

  mochaOpts: {
    ui: "bdd",
    timeout: 60000,
  },
};
