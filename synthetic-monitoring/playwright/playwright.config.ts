// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './specs',
  fullyParallel: false, // synthetic — serialize for consistent metrics
  retries: 2,
  workers: 1,
  reporter: [['list'], ['json', { outputFile: 'results.json' }]],
  use: {
    baseURL: process.env.FASO_PROD_URL || 'https://staging.faso.gov.bf',
    headless: true,
    trace: 'retain-on-failure',
    video: 'retain-on-failure',
    screenshot: 'only-on-failure',
    actionTimeout: 15_000,
    navigationTimeout: 30_000,
  },
  projects: [
    { name: 'chromium', use: { ...devices['Desktop Chrome'] } },
  ],
  timeout: 60_000,
});
