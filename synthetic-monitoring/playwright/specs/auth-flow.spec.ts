// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
// Synthetic monitoring: login → dashboard → logout flow.
// Runs every 5 min via cron, pushes metrics to Prometheus Pushgateway.

import { test, expect } from '@playwright/test';
import { pushMetrics } from '../lib/push-metrics';

const SCENARIO = 'auth_flow';

test('citizen login → dashboard → logout', async ({ page }) => {
  const start = Date.now();
  let success = false;
  try {
    await page.goto(process.env.FASO_PROD_URL || 'https://staging.faso.gov.bf');
    await page.getByRole('button', { name: /connexion|login/i }).click();
    await page.getByLabel('Email').fill(process.env.SYNTHETIC_USER_EMAIL!);
    await page.getByLabel('Mot de passe').fill(process.env.SYNTHETIC_USER_PASSWORD!);
    await page.getByRole('button', { name: /se connecter|sign in/i }).click();

    await expect(page.getByText(/tableau de bord|dashboard/i)).toBeVisible({ timeout: 10_000 });

    await page.getByRole('button', { name: /déconnexion|logout/i }).click();
    await expect(page.getByRole('button', { name: /connexion|login/i })).toBeVisible();

    success = true;
  } finally {
    const durationMs = Date.now() - start;
    await pushMetrics(SCENARIO, { duration_ms: durationMs, success: success ? 1 : 0 });
  }
});
