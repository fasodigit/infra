// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec P1.5 — Agent terrain offline sync roundtrip via mobile-bff.
 *
 * Valide :
 *   - Pré-requis : 5 producteurs + 5 parcelles existants (créés par la spec).
 *   - POST /api/terroir/mobile-bff/m/sync/batch avec un batch de 50 items :
 *     5 producer-update + 5 parcel-update + 40 parcel-polygon-update.
 *   - Réponse 200 + acks.length === 50, **chaque** ack `status="ok"` ou
 *     `status="error"` (pas de timeout, pas de 5xx).
 *   - Au moins 90% des items doivent avoir `status="ok"` (budget P99
 *     EDGE = sync 50 items < 2 min).
 *   - Idempotency : ré-envoyer le même `batchId` → 409 Conflict.
 *   - Erreur : POST avec `items=[]` → 400.
 *
 * Note SYNC_BATCH_MAX_ITEMS : terroir-mobile-bff impose `SYNC_BATCH_MAX_ITEMS
 * = 100` ; on reste bien sous le plafond avec 50.
 */
import { test, expect } from '@playwright/test';
import { CoreClient } from '../../fixtures/terroir/core-client';
import { MobileBffClient } from '../../fixtures/terroir/mobile-bff-client';

const TENANT_SLUG = process.env.TERROIR_TENANT_SLUG ?? 't_pilot';
const COOP_PILOT_UUID =
  process.env.TERROIR_COOP_PILOT_UUID ?? '00000000-0000-4000-8000-00000coopilot';

const SYNC_P99_BUDGET_MS = 2 * 60_000; // 2 minutes — ULTRAPLAN P1 acceptance

test.describe('TERROIR P1.5 — agent offline sync roundtrip 50 items', () => {
  let core: CoreClient;
  let bff: MobileBffClient;
  let reachable = false;

  test.beforeAll(async () => {
    core = new CoreClient({ tenantSlug: TENANT_SLUG });
    bff = new MobileBffClient({ tenantSlug: TENANT_SLUG });
    reachable = (await core.isReachable()) && (await bff.isReachable());
  });

  test.beforeEach(async ({}, testInfo) => {
    if (!reachable) {
      testInfo.skip(
        true,
        'ARMAGEDDON :8080 unreachable — terroir-mobile-bff stack-down. Run /cycle-fix first.',
      );
    }
  });

  test.setTimeout(180_000); // 3 min — sync work + setup creations.

  test('happy path — 50 items batch sync within EDGE 2 min budget', async () => {
    // Step 0 : create 5 producers + 5 parcels prerequis.
    const producerIds: string[] = [];
    const parcelIds: string[] = [];
    for (let i = 0; i < 5; i++) {
      const pr = await core.createProducer({
        cooperativeId: COOP_PILOT_UUID,
        fullName: `Sync-Producer-${i}-${Date.now()}`,
        nin: `BF-${Math.floor(Math.random() * 1e10).toString().padStart(10, '0')}`,
        phone: `+22670${Math.floor(Math.random() * 1e6).toString().padStart(6, '0')}`,
        gpsDomicileLat: 12.0 + i * 0.01,
        gpsDomicileLon: -3.0 + i * 0.01,
        primaryCrop: i % 2 === 0 ? 'coton' : 'mais',
      });
      expect([200, 201]).toContain(pr.status);
      const pid = (pr.body as { id: string }).id;
      producerIds.push(pid);
      const pa = await core.createParcel({
        producerId: pid,
        cropType: i % 2 === 0 ? 'coton' : 'mais',
        surfaceHectares: 0.5 + i * 0.1,
      });
      expect([200, 201]).toContain(pa.status);
      parcelIds.push((pa.body as { id: string }).id);
    }

    // Step 1 : build & POST 50-items batch.
    const batch = MobileBffClient.makeSyntheticBatch({
      producerIds,
      parcelIds,
      counts: { producerUpdates: 5, parcelUpdates: 5, polygonUpdates: 40 },
    });
    expect(batch.items.length).toBe(50);

    const t0 = Date.now();
    const res = await bff.syncBatch(batch);
    const elapsed = Date.now() - t0;

    expect(res.status).toBe(200);
    const body = res.body as { batchId: string; acks: { status: string }[] };
    expect(body.batchId).toBe(batch.batchId);
    expect(body.acks.length).toBe(50);
    expect(elapsed).toBeLessThan(SYNC_P99_BUDGET_MS);

    // ≥ 90% des items doivent avoir réussi (P1 acceptance — 10% tolérance
    // pour LWW conflicts internes du synthetic batch).
    const oks = body.acks.filter((a) => a.status === 'ok').length;
    expect(oks / 50).toBeGreaterThanOrEqual(0.9);

    // Step 2 : idempotency — re-send same batchId → 409.
    const dup = await bff.syncBatch(batch);
    expect(dup.status).toBe(409);
  });

  test('error — empty batch → 400', async () => {
    const res = await bff.syncBatch({
      batchId: '00000000-0000-4000-8000-000000000000',
      items: [],
    });
    expect(res.status).toBeGreaterThanOrEqual(400);
    expect(res.status).toBeLessThan(500);
  });
});
