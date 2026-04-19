// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
// Synthetic flow: login → dashboard → logout.
// SLA: end-to-end < 3 seconds.

import { test, expect } from '@playwright/test';
import { syntheticUser, FLOWS } from '../fixtures/synthetic-user';
import { collectPerfMetrics, measureStep, trackHttp5xx } from '../helpers/timing';
import {
  pushSyntheticReport,
  type StepTiming,
} from '../helpers/prometheus-push';

const SLA_MS = 3_000;

test.describe('auth flow', () => {
  test('login < 3s then logout', async ({ page }) => {
    const user = syntheticUser();
    const http5xx = trackHttp5xx(page);
    const steps: StepTiming[] = [];
    const started = Date.now();
    let success = false;

    try {
      const goto = await measureStep('goto_landing', async () => {
        await page.goto('/');
        await expect(page).toHaveTitle(/FASO|Plateforme/i);
      });
      steps.push(goto);

      const login = await measureStep('login', async () => {
        await page.getByRole('button', { name: /connexion|login/i }).click();
        await page.getByLabel(/email|courriel/i).fill(user.email);
        await page.getByLabel(/mot de passe|password/i).fill(user.password);
        await page.getByRole('button', { name: /se connecter|sign in/i }).click();
        await expect(
          page.getByText(/tableau de bord|dashboard|bienvenue/i),
        ).toBeVisible({ timeout: 10_000 });
      });
      steps.push(login);

      const logout = await measureStep('logout', async () => {
        await page.getByRole('button', { name: /déconnexion|logout/i }).click();
        await expect(
          page.getByRole('button', { name: /connexion|login/i }),
        ).toBeVisible();
      });
      steps.push(logout);

      const totalMs = Date.now() - started;
      expect(totalMs, `auth flow must complete under ${SLA_MS}ms`).toBeLessThan(
        SLA_MS,
      );
      success = true;
    } finally {
      const totalDurationMs = Date.now() - started;
      const perf = await collectPerfMetrics(page, http5xx.count()).catch(() => undefined);
      await pushSyntheticReport({
        flow: FLOWS.AUTH,
        success,
        totalDurationMs,
        steps,
        perf,
      });
    }
  });
});
