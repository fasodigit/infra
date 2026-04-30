// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec P1.3 — EUDR validation happy path (clean polygon).
 *
 * Valide :
 *   - 1ʳᵉ POST /eudr/validate avec un polygon BF Boucle du Mouhoun "clean"
 *     (zone agricole connue sans déforestation post-2020) →
 *     `status=VALIDATED`, header `X-Eudr-Cache-Status: MISS`,
 *     `deforestationOverlapHa` ≈ 0 (≤ seuil minimal).
 *   - 2ᵉ POST identique → header `X-Eudr-Cache-Status: HIT` (cache KAYA
 *     prefixé `eudr:cache:{tenant}:{polygonHash}`).
 *   - GET /eudr/parcels/{id}/validations → contient au moins une entrée.
 *   - Erreur : POST avec polygon GeoJSON malformé → 400.
 *
 * Assertion key : VALIDATED + cache MISS→HIT + dataset_version non-vide.
 */
import { test, expect } from '@playwright/test';
import { CoreClient } from '../../fixtures/terroir/core-client';
import { EudrClient, bfCleanPolygon } from '../../fixtures/terroir/eudr-client';

const TENANT_SLUG = process.env.TERROIR_TENANT_SLUG ?? 't_pilot';
const COOP_PILOT_UUID =
  process.env.TERROIR_COOP_PILOT_UUID ?? '00000000-0000-4000-8000-00000coopilot';

test.describe('TERROIR P1.3 — EUDR validation happy path', () => {
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
        'ARMAGEDDON :8080 unreachable — terroir-eudr stack-down. Run /cycle-fix first.',
      );
    }
  });

  test('happy path — clean polygon validates, cache MISS then HIT', async () => {
    // Step 0 : create producer + parcel that will be validated.
    const producerRes = await core.createProducer({
      cooperativeId: COOP_PILOT_UUID,
      fullName: `Producer-EUDR-OK-${Date.now()}`,
      nin: `BF-${Math.floor(Math.random() * 1e10)}`,
      phone: `+22670${Math.floor(Math.random() * 1e6).toString().padStart(6, '0')}`,
      gpsDomicileLat: 12.521,
      gpsDomicileLon: -3.027,
      primaryCrop: 'coton',
    });
    expect([200, 201]).toContain(producerRes.status);
    const producerId = (producerRes.body as { id: string }).id;

    const parcelRes = await core.createParcel({
      producerId,
      cropType: 'coton',
      surfaceHectares: 0.36,
    });
    expect([200, 201]).toContain(parcelRes.status);
    const parcelId = (parcelRes.body as { id: string }).id;

    const polygon = bfCleanPolygon(12.521, -3.027);

    // Step 1 : 1st validate → MISS.
    const v1 = await eudr.validate({ parcelId, polygonGeoJson: polygon });
    expect(v1.status).toBe(200);
    const r1 = v1.body as {
      status: string;
      datasetVersion: string;
      polygonHash: string;
      deforestationOverlapHa: number;
    };
    expect(r1.status).toBe('VALIDATED');
    expect(r1.datasetVersion).not.toBe('');
    expect(r1.polygonHash).toMatch(/^[0-9a-f]{32,}$/i);
    expect(r1.deforestationOverlapHa).toBeLessThanOrEqual(0.05);
    // Header X-Eudr-Cache-Status: MISS on first call (case-insensitive).
    const ch1 = (v1.headers['x-eudr-cache-status'] ?? '').toUpperCase();
    expect(ch1).toBe('MISS');

    // Step 2 : 2nd validate (identical polygon) → HIT.
    const v2 = await eudr.validate({ parcelId, polygonGeoJson: polygon });
    expect(v2.status).toBe(200);
    const r2 = v2.body as { status: string; polygonHash: string };
    expect(r2.status).toBe('VALIDATED');
    expect(r2.polygonHash).toBe(r1.polygonHash);
    const ch2 = (v2.headers['x-eudr-cache-status'] ?? '').toUpperCase();
    expect(ch2).toBe('HIT');

    // Step 3 : GET validations history.
    const list = await eudr.listValidations(parcelId);
    expect(list.status).toBe(200);
    const items = (list.body as { items: unknown[] }).items;
    expect(items.length).toBeGreaterThanOrEqual(1);
  });

  test('error — invalid GeoJSON (no coordinates) → 4xx', async () => {
    // Need a parcel id that actually exists so the failure is at the
    // GeoJSON layer, not the parcel-not-found layer.
    const producerRes = await core.createProducer({
      cooperativeId: COOP_PILOT_UUID,
      fullName: `Producer-Bad-Poly-${Date.now()}`,
      nin: `BF-${Math.floor(Math.random() * 1e10)}`,
      phone: `+22670${Math.floor(Math.random() * 1e6).toString().padStart(6, '0')}`,
      gpsDomicileLat: 12.0,
      gpsDomicileLon: -3.0,
      primaryCrop: 'coton',
    });
    expect([200, 201]).toContain(producerRes.status);
    const producerId = (producerRes.body as { id: string }).id;
    const parcelRes = await core.createParcel({ producerId });
    expect([200, 201]).toContain(parcelRes.status);
    const parcelId = (parcelRes.body as { id: string }).id;

    const malformed = { type: 'Feature', geometry: { type: 'Polygon' } };
    const res = await eudr.validate({
      parcelId,
      polygonGeoJson: malformed,
    });
    expect(res.status).toBeGreaterThanOrEqual(400);
    expect(res.status).toBeLessThan(500);
  });
});
