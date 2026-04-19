// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
// Synthetic flow: demande de certificat d'état civil.
// Le service etat-civil-ms peut ne pas être déployé encore :
// dans ce cas le spec se met en mode "stub" et produit quand même
// les métriques avec success=0 + step "service_unavailable".

import { test, expect } from '@playwright/test';
import { syntheticUser, FLOWS } from '../fixtures/synthetic-user';
import { collectPerfMetrics, measureStep, trackHttp5xx } from '../helpers/timing';
import {
  pushSyntheticReport,
  type StepTiming,
} from '../helpers/prometheus-push';

const SERVICE_READY = process.env.ETAT_CIVIL_READY === 'true';

test.describe('etat-civil flow', () => {
  test('demande certificat de naissance', async ({ page }) => {
    const user = syntheticUser();
    const http5xx = trackHttp5xx(page);
    const steps: StepTiming[] = [];
    const started = Date.now();
    let success = false;

    try {
      if (!SERVICE_READY) {
        steps.push({ label: 'service_unavailable_stub', durationMs: 1 });
        test.skip(
          true,
          'etat-civil service not yet deployed — stub run, metrics reported.',
        );
        return;
      }

      steps.push(
        await measureStep('goto_etat_civil', async () => {
          await page.goto('/etat-civil');
          await expect(
            page.getByRole('heading', { name: /état civil/i }),
          ).toBeVisible();
        }),
      );

      steps.push(
        await measureStep('login', async () => {
          await page.getByRole('button', { name: /connexion|login/i }).click();
          await page.getByLabel(/email|courriel/i).fill(user.email);
          await page.getByLabel(/mot de passe|password/i).fill(user.password);
          await page.getByRole('button', { name: /se connecter|sign in/i }).click();
        }),
      );

      steps.push(
        await measureStep('submit_request', async () => {
          await page
            .getByRole('link', { name: /acte de naissance/i })
            .click();
          await page.getByLabel(/numéro nina|NIP/i).fill('SYNTHETIC-000000');
          await page.getByLabel(/motif/i).fill('Monitoring synthétique');
          await page
            .getByRole('button', { name: /soumettre|submit/i })
            .click();
          await expect(
            page.getByText(/demande enregistrée|request saved/i),
          ).toBeVisible({ timeout: 15_000 });
        }),
      );

      success = true;
    } finally {
      const totalDurationMs = Date.now() - started;
      const perf = await collectPerfMetrics(page, http5xx.count()).catch(() => undefined);
      await pushSyntheticReport({
        flow: FLOWS.ETAT_CIVIL,
        success,
        totalDurationMs,
        steps,
        perf,
      });
    }
  });
});
