import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  reporter: [["list"]],
  timeout: 180_000,
  expect: {
    timeout: 15_000,
  },
  use: {
    baseURL: "http://localhost:1421",
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
  webServer: {
    command: "npm run dev",
    url: "http://localhost:1421",
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
  },
});
