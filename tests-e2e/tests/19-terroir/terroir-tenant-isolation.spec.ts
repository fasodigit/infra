// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec P1.1 (isolation) — Tenant isolation cross-tenant via Keto + RLS.
 *
 * Valide :
 *   - Tenant A (`t_pilot`) crée un producteur P_A via X-Tenant-Slug=t_pilot.
 *   - Tenant B (`t_e2e_iso_*`, créé via terroir-admin) tente de GET P_A en
 *     présentant son propre `X-Tenant-Slug=t_e2e_iso_*` → 404 (le
 *     producteur n'existe PAS dans le schéma de B) ou 403 (Keto refuse).
 *   - Probe SQL direct via le rôle `terroir_app` sur `terroir_t_t_e2e_iso_*`.
 *     producer → 0 rows visibles (aucune fuite de données).
 *   - Probe SQL sur `terroir_t_t_pilot.producer` pour assert que P_A
 *     existe bien dans son tenant d'origine (zero false-negative).
 *   - Erreur : POST avec `X-Tenant-Slug` invalide → 400/401.
 *
 * Cleanup : suspend le tenant E2E créé pour éviter la pollution.
 */
import { test, expect } from '@playwright/test';
import { CoreClient } from '../../fixtures/terroir/core-client';
import { TenantAdminClient } from '../../fixtures/terroir/tenant-admin-client';
import { PgProbe } from '../../fixtures/terroir/pg-probe';

const TENANT_A_SLUG = process.env.TERROIR_TENANT_SLUG ?? 't_pilot';
const COOP_PILOT_UUID =
  process.env.TERROIR_COOP_PILOT_UUID ?? '00000000-0000-4000-8000-00000coopilot';

function randIsoSlug(): string {
  const stamp = Date.now().toString(36);
  const rand = Math.floor(Math.random() * 1e6).toString(36);
  return `t_iso_${stamp}_${rand}`.slice(0, 32);
}

test.describe('TERROIR P1 — tenant isolation (Keto + RLS)', () => {
  let coreA: CoreClient;
  let admin: TenantAdminClient;
  let pg: PgProbe;
  let reachable = false;

  test.beforeAll(async () => {
    coreA = new CoreClient({ tenantSlug: TENANT_A_SLUG });
    admin = new TenantAdminClient();
    pg = new PgProbe();
    reachable = (await coreA.isReachable()) && (await admin.isReachable());
  });

  test.beforeEach(async ({}, testInfo) => {
    if (!reachable) {
      testInfo.skip(
        true,
        'ARMAGEDDON :8080 or terroir-admin :9904 unreachable — run /cycle-fix first.',
      );
    }
  });

  test('happy path — tenant B cannot read tenant A producer', async () => {
    // Step 0 : provision a fresh tenant B.
    const tenantBSlug = randIsoSlug();
    const provision = await admin.createTenant({
      slug: tenantBSlug,
      legal_name: `Coopérative ISO ${tenantBSlug}`,
      country_iso2: 'BF',
      region: 'Centre',
      primary_crop: 'coton',
    });
    expect(provision.status).toBe(201);

    // Step 1 : create producer in tenant A.
    const aRes = await coreA.createProducer({
      cooperativeId: COOP_PILOT_UUID,
      fullName: `ISO-Cross-Producer-${Date.now()}`,
      nin: `BF-${Math.floor(Math.random() * 1e10)}`,
      phone: `+22670${Math.floor(Math.random() * 1e6).toString().padStart(6, '0')}`,
      gpsDomicileLat: 12.0,
      gpsDomicileLon: -3.0,
      primaryCrop: 'coton',
    });
    expect([200, 201]).toContain(aRes.status);
    const producerA = (aRes.body as { id: string }).id;

    // Step 2 : tenant B tries to GET that producer → must 4xx (not 200).
    const coreB = new CoreClient({ tenantSlug: tenantBSlug });
    const bView = await coreB.getProducer(producerA);
    expect(bView.status).toBeGreaterThanOrEqual(400);
    expect(bView.status).toBeLessThan(500);
    // Specific contract: 404 (schema isolation) or 403 (Keto deny).
    expect([403, 404]).toContain(bView.status);

    // Step 3 : SQL probe via terroir_app on tenant B schema must NOT see
    // producer A (RLS / schema isolation enforced).
    const tenantBSchema = `terroir_t_${tenantBSlug}`;
    const tenantASchema = `terroir_t_${TENANT_A_SLUG}`;
    const bRows = await pg.runOne(
      `SELECT COUNT(*)::int AS c FROM ${tenantBSchema}.producer WHERE id = $1`,
      [producerA],
    );
    if (bRows.unavailable) {
      test.info().annotations.push({
        type: 'pg-probe-skip',
        description: bRows.reason ?? 'unknown',
      });
    } else {
      expect(bRows.rows).toBeDefined();
      expect((bRows.rows![0] as { c: number }).c).toBe(0);
    }

    // Confirm zero false-negative: producer A is visible in its own schema.
    const aRows = await pg.runOne(
      `SELECT COUNT(*)::int AS c FROM ${tenantASchema}.producer WHERE id = $1`,
      [producerA],
    );
    if (!aRows.unavailable) {
      expect(aRows.rows).toBeDefined();
      expect((aRows.rows![0] as { c: number }).c).toBe(1);
    }

    // Step 4 : list producers from tenant B → producer A must NOT appear.
    const bList = await coreB.listProducers({ size: 100 });
    expect(bList.status).toBe(200);
    const items = (bList.body as { items: { id: string }[] }).items;
    expect(items.find((i) => i.id === producerA)).toBeUndefined();

    // Cleanup : suspend tenant B so it doesn't pollute future runs.
    await admin.suspendTenant(tenantBSlug).catch(() => undefined);
  });

  test('error — invalid X-Tenant-Slug header → 4xx', async () => {
    const bogus = new CoreClient({ tenantSlug: 'INVALID UPPER 🚫' });
    const res = await bogus.listProducers();
    expect(res.status).toBeGreaterThanOrEqual(400);
    expect(res.status).toBeLessThan(500);
  });
});
