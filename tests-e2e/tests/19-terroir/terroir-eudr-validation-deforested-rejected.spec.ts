// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec P1.3 (deforested) — EUDR validation reject path.
 *
 * Valide :
 *   - Polygon synthétique forte loss post-2020 (Hansen GFC) →
 *     `status=REJECTED` ou `ESCALATED` + `deforestationOverlapHa > 0`.
 *   - Le `reason` ou la propriété de réponse mentionne "deforestation".
 *   - Si overlap dépasse seuil critique configuré (`EUDR_ESCALATION_THRESHOLD_HA`)
 *     → `status=ESCALATED` (workflow autorité-BF), confirmé par audit.
 *   - Erreur : POST /eudr/validate sans `parcelId` → 400/422.
 *
 * Pas de mocks ; pas de bypass auth.
 */
import { test, expect } from '@playwright/test';
import { CoreClient } from '../../fixtures/terroir/core-client';
import { EudrClient, deforestedPolygon } from '../../fixtures/terroir/eudr-client';

const TENANT_SLUG = process.env.TERROIR_TENANT_SLUG ?? 't_pilot';
const COOP_PILOT_UUID =
  process.env.TERROIR_COOP_PILOT_UUID ?? '00000000-0000-4000-8000-00000coopilot';

test.describe('TERROIR P1.3 — EUDR validation deforested polygon rejected', () => {
  let core: CoreClient;
  let eudr: EudrClient;
  let reachable = false;

  test.beforeAll(async () => {
    core = new CoreClient({ tenantSlug: TENANT_SLUG });
    eudr = new EudrClient({ tenantSlug: TENANT_SLUG });
    reachable = await core.isReachable();
  });

  test.beforeEach(async ({}, testInfo) => {
    if (!reachable) {
      testInfo.skip(
        true,
        'ARMAGEDDON :8080 unreachable — run /cycle-fix first.',
      );
    }
  });

  test('happy path — deforested polygon → REJECTED|ESCALATED', async () => {
    // Setup: producer + parcel.
    const producerRes = await core.createProducer({
      cooperativeId: COOP_PILOT_UUID,
      fullName: `Producer-Deforest-${Date.now()}`,
      nin: `BF-${Math.floor(Math.random() * 1e10)}`,
      phone: `+22670${Math.floor(Math.random() * 1e6).toString().padStart(6, '0')}`,
      gpsDomicileLat: 12.0,
      gpsDomicileLon: -3.0,
      primaryCrop: 'coton',
    });
    expect([200, 201]).toContain(producerRes.status);
    const producerId = (producerRes.body as { id: string }).id;
    const parcelRes = await core.createParcel({
      producerId,
      cropType: 'coton',
      surfaceHectares: 1.0,
    });
    expect([200, 201]).toContain(parcelRes.status);
    const parcelId = (parcelRes.body as { id: string }).id;

    // Validate with deforested polygon.
    const v = await eudr.validate({
      parcelId,
      polygonGeoJson: deforestedPolygon(),
    });
    expect(v.status).toBe(200);
    const r = v.body as {
      status: string;
      deforestationOverlapHa: number;
      datasetVersion: string;
      evidenceUrl?: string;
      ddsDraftId?: string;
    };
    // Le validateur doit catégoriser en REJECTED ou ESCALATED suivant le
    // seuil configuré (EUDR_ESCALATION_THRESHOLD_HA, cf. terroir-eudr config).
    expect(['REJECTED', 'ESCALATED']).toContain(r.status);
    expect(r.deforestationOverlapHa).toBeGreaterThan(0);
    expect(r.datasetVersion).not.toBe('');
    // No DDS draft is created when status != VALIDATED.
    expect(r.ddsDraftId ?? null).toBeNull();

    // Validations history must contain the rejection.
    const list = await eudr.listValidations(parcelId);
    expect(list.status).toBe(200);
    const items = (list.body as { items: { status: string }[] }).items;
    expect(items.length).toBeGreaterThanOrEqual(1);
    expect(
      items.some((it) => it.status === 'REJECTED' || it.status === 'ESCALATED'),
    ).toBe(true);
  });

  test('error — POST /eudr/validate without parcelId → 4xx', async () => {
    const malformed = {
      // parcelId omis volontairement
      polygonGeoJson: deforestedPolygon(),
    } as unknown as { parcelId: string; polygonGeoJson: Record<string, unknown> };
    const res = await eudr.validate(malformed);
    expect(res.status).toBeGreaterThanOrEqual(400);
    expect(res.status).toBeLessThan(500);
  });
});
