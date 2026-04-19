// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
// Playwright configuration — production synthetic monitoring.
// Each flow measures navigation timing, FCP/LCP/TTI and pushes Prometheus
// metrics to the Pushgateway.

import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './playwright/flows',
  fullyParallel: false,
  retries: 1,
  workers: 1,
  reporter: [
    ['list'],
    ['json', { outputFile: 'results.json' }],
    ['html', { outputFolder: 'playwright-report', open: 'never' }],
  ],
  use: {
    baseURL: process.env.FASO_PROD_URL || 'https://staging.faso.gov.bf',
    headless: true,
    trace: 'retain-on-failure',
    video: 'retain-on-failure',
    screenshot: 'only-on-failure',
    actionTimeout: 15_000,
    navigationTimeout: 30_000,
    extraHTTPHeaders: {
      'X-Synthetic-Monitor': 'faso-digitalisation',
    },
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  timeout: 120_000,
  expect: {
    timeout: 10_000,
  },
});
