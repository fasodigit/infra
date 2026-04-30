// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec P1.3 (DDS) — Génération + signature Vault PKI + soumission TRACES NT.
 *
 * Valide :
 *   - Pré-requis : producer + parcel + validation VALIDATED (clean polygon).
 *   - POST /eudr/dds → DDS généré + payloadSha256 + status="generated".
 *   - POST /eudr/dds/{id}/sign → signatureFingerprint hex + status="signed".
 *   - POST /eudr/dds/{id}/submit → mock TRACES NT renvoie 200, status="submitted",
 *     `tracesNtRef` non-vide, attemptNo ≥ 1.
 *   - GET  /eudr/dds/{id}/download → Content-Type application/pdf + magic
 *     bytes `%PDF-` au début du buffer.
 *   - Erreur : sign sur DDS inexistant → 404.
 *
 * Pas de mocks Playwright ; le mock TRACES NT est côté serveur (provider
 * stub://) — c'est un real backend pour la spec.
 */
import { test, expect } from '@playwright/test';
import { CoreClient } from '../../fixtures/terroir/core-client';
import { EudrClient, bfCleanPolygon } from '../../fixtures/terroir/eudr-client';

const TENANT_SLUG = process.env.TERROIR_TENANT_SLUG ?? 't_pilot';
const COOP_PILOT_UUID =
  process.env.TERROIR_COOP_PILOT_UUID ?? '00000000-0000-4000-8000-00000coopilot';

test.describe('TERROIR P1.3 — DDS generation + signature + submission', () => {
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
      testInfo.skip(true, 'ARMAGEDDON :8080 unreachable — run /cycle-fix first.');
    }
  });

  test('happy path — generate → sign → submit → download PDF', async () => {
    // Step 0 : producer + parcel + validation VALIDATED.
    const producerRes = await core.createProducer({
      cooperativeId: COOP_PILOT_UUID,
      fullName: `Exporter-${Date.now()}`,
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
      surfaceHectares: 1.2,
    });
    expect([200, 201]).toContain(parcelRes.status);
    const parcelId = (parcelRes.body as { id: string }).id;

    const valRes = await eudr.validate({
      parcelId,
      polygonGeoJson: bfCleanPolygon(12.521, -3.027),
    });
    expect(valRes.status).toBe(200);
    const validation = valRes.body as {
      validationId: string;
      status: string;
    };
    expect(validation.status).toBe('VALIDATED');

    // Step 1 : generate DDS.
    const genRes = await eudr.generateDds({
      validationId: validation.validationId,
      hsCode: '5201',
      quantity: 1500,
      unit: 'kg',
      countryIso2: 'BF',
      harvestPeriod: '2026-Q1',
    });
    expect(genRes.status).toBeGreaterThanOrEqual(200);
    expect(genRes.status).toBeLessThan(300);
    const dds = genRes.body as {
      ddsId: string;
      status: string;
      payloadSha256: string;
    };
    expect(dds.ddsId).toMatch(/^[0-9a-f-]{36}$/);
    expect(dds.payloadSha256).toMatch(/^[0-9a-f]{64}$/i);

    // Step 2 : sign via Vault PKI.
    const signRes = await eudr.signDds(dds.ddsId);
    expect(signRes.status).toBe(200);
    const signed = signRes.body as {
      signatureFingerprint: string;
      status: string;
    };
    expect(signed.signatureFingerprint.length).toBeGreaterThan(20);
    expect(signed.status).toBe('signed');

    // Step 3 : submit to TRACES NT (mock provider).
    const subRes = await eudr.submitDds(dds.ddsId);
    expect(subRes.status).toBe(200);
    const sub = subRes.body as {
      status: string;
      tracesNtRef?: string;
      attemptNo: number;
    };
    expect(sub.status).toBe('submitted');
    expect(sub.tracesNtRef ?? '').not.toBe('');
    expect(sub.attemptNo).toBeGreaterThanOrEqual(1);

    // Step 4 : download PDF binary.
    const dl = await eudr.downloadDds(dds.ddsId);
    expect(dl.status).toBe(200);
    expect(dl.contentType).toContain('application/pdf');
    // Magic bytes "%PDF-" (0x25 0x50 0x44 0x46 0x2D).
    expect(dl.bytes.slice(0, 5).toString('utf8')).toBe('%PDF-');
    expect(dl.bytes.length).toBeGreaterThan(200);
  });

  test('error — sign DDS that does not exist → 404', async () => {
    const fakeId = '00000000-0000-4000-8000-deadbeefdead';
    const r = await eudr.signDds(fakeId);
    expect([400, 404]).toContain(r.status);
  });
});
