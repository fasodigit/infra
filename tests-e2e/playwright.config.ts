import { defineConfig, devices } from '@playwright/test';

// Default baseURL = ARMAGEDDON sovereign gateway (8080).
// All E2E traffic must enter through the gateway: this validates routing,
// JWT/JWKS cache, OPA ext_authz, Coraza WAF, rate limiting, and OTel
// trace propagation as a cohesive pipeline.
//
// Override with BASE_URL=http://localhost:4801 to bypass the gateway and
// hit the Angular dev server directly (smoke-only, NOT a sovereign run).
const BASE_URL = process.env.BASE_URL ?? 'http://localhost:8080';
const WORKERS = Number(process.env.PW_WORKERS ?? 4);
const CI = !!process.env.CI;

export default defineConfig({
  testDir: './tests',
  fullyParallel: true,
  forbidOnly: CI,
  retries: CI ? 2 : 0,
  workers: WORKERS,
  timeout: 60_000,
  expect: {
    timeout: 10_000,
  },
  globalSetup: './fixtures/global-setup.ts',
  reporter: [
    ['list'],
    ['html', { outputFolder: 'reports/html', open: 'never' }],
    ['json', { outputFile: 'reports/results.json' }],
    ['junit', { outputFile: 'reports/junit.xml' }],
  ],
  outputDir: 'test-results',
  use: {
    baseURL: BASE_URL,
    headless: true,
    locale: 'fr-BF',
    timezoneId: 'Africa/Ouagadougou',
    viewport: { width: 1440, height: 900 },
    actionTimeout: 10_000,
    navigationTimeout: 30_000,
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
    contextOptions: {
      recordHar: { path: 'reports/har/trace.har', mode: 'minimal' },
    },
    ignoreHTTPSErrors: false,
    colorScheme: 'light',
  },
  projects: [
    {
      name: 'chromium-headless',
      use: {
        ...devices['Desktop Chrome'],
        headless: true,
      },
    },
    {
      name: 'chrome-smoke',
      use: {
        ...devices['Desktop Chrome'],
        channel: 'chrome',
        headless: true,
      },
      grep: /@smoke/,
    },
    {
      name: 'chrome-headless-new',
      use: {
        ...devices['Desktop Chrome'],
        channel: 'chrome',
        headless: true,
        viewport: { width: 1440, height: 900 },
        launchOptions: {
          args: [
            '--headless=new',
            '--disable-gpu',
            '--no-sandbox',
            '--disable-dev-shm-usage',
            '--disable-blink-features=AutomationControlled',
            '--enable-features=NetworkService,NetworkServiceInProcess',
          ],
        },
      },
    },
  ],
});
