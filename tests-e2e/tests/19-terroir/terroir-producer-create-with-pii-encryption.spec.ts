// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec P1.1 — Création producteur via terroir-core + assertion chiffrement
 * PII en base.
 *
 * Valide :
 *   - POST /api/terroir/core/producers (via ARMAGEDDON :8080) → 201 + body.
 *   - GET  /api/terroir/core/producers/{id} → 200 + plaintext déchiffré côté
 *     service (le client voit `fullName` clair, mais en DB c'est chiffré).
 *   - SQL probe direct : `terroir_t_t_pilot.producer.full_name_encrypted IS
 *     NOT NULL` ET `nin_encrypted IS NOT NULL` (proof PII at rest = ciphertext).
 *   - Erreur : POST sans `cooperativeId` → 400.
 *
 * Acteur principal : Aminata SUPER-ADMIN (seededSuperAdmins[0]). Pour la P1
 * MVP on utilise l'auth M2M `X-Tenant-Slug` car le flow JWT producer-create
 * passe en P1.6 (mobile RN). La spec couvre quand même le path real backend.
 *
 * Aucun mock ; aucun bypass auth via cookie forgé.
 */
import { test, expect } from '@playwright/test';
import { CoreClient, type ProducerCreateRequest } from '../../fixtures/terroir/core-client';
import { PgProbe } from '../../fixtures/terroir/pg-probe';
import { seededSuperAdmins } from '../../fixtures/actors';

const TENANT_SLUG = process.env.TERROIR_TENANT_SLUG ?? 't_pilot';
const COOP_PILOT_UUID =
  process.env.TERROIR_COOP_PILOT_UUID ?? '00000000-0000-4000-8000-00000coopilot';

function randNin(): string {
  return `BF-${Math.floor(Math.random() * 1e10).toString().padStart(10, '0')}`;
}

function randPhone(): string {
  return `+22670${Math.floor(Math.random() * 1e6).toString().padStart(6, '0')}`;
}

test.describe('TERROIR P1.1 — producer create + PII encryption', () => {
  let core: CoreClient;
  let pg: PgProbe;
  let coreReachable = false;

  test.beforeAll(async () => {
    core = new CoreClient({ tenantSlug: TENANT_SLUG });
    pg = new PgProbe();
    coreReachable = await core.isReachable();
  });

  test.beforeEach(async ({}, testInfo) => {
    if (!coreReachable) {
      testInfo.skip(
        true,
        'ARMAGEDDON :8080 unreachable — stack-down. Run /cycle-fix first.',
      );
    }
  });

  test('happy path — POST producer + DB ciphertext + GET roundtrip', async () => {
    const aminata = seededSuperAdmins[0]!;
    const payload: ProducerCreateRequest = {
      cooperativeId: COOP_PILOT_UUID,
      fullName: `${aminata.firstName} TEST-PII ${Date.now()}`,
      nin: randNin(),
      phone: randPhone(),
      gpsDomicileLat: 12.371,
      gpsDomicileLon: -1.519,
      primaryCrop: 'coton',
    };

    // 1. Create
    const created = await core.createProducer(payload);
    expect(created.status).toBe(201);
    const producer = created.body as { id: string; fullName: string };
    expect(producer.id).toMatch(/^[0-9a-f-]{36}$/);
    expect(producer.fullName).toContain('TEST-PII');

    // 2. SQL probe — assert ciphertext at rest.
    const tenantSchema = `terroir_t_${TENANT_SLUG}`;
    const probe = await pg.assertPiiEncrypted(tenantSchema, producer.id);
    if (probe.unavailable) {
      test.info().annotations.push({
        type: 'skip-reason',
        description: `pg-probe-unavailable: ${probe.reason ?? 'unknown'}`,
      });
      test.skip(true, 'PG probe unavailable — skipping ciphertext assertion');
    }
    expect(probe.rows).toBeDefined();
    expect(probe.rows!.length).toBe(1);
    const row = probe.rows![0]!;
    // BYTEA columns must not be null AND must NOT contain the cleartext
    // (would mean encryption was bypassed).
    expect(row.full_name_encrypted).not.toBeNull();
    expect(row.nin_encrypted).not.toBeNull();
    const nameAsBytes = Buffer.isBuffer(row.full_name_encrypted)
      ? (row.full_name_encrypted as Buffer)
      : Buffer.from(String(row.full_name_encrypted));
    expect(nameAsBytes.toString('utf8')).not.toContain('TEST-PII');

    // 3. GET roundtrip — service decrypts → plaintext recovered.
    const fetched = await core.getProducer(producer.id);
    expect(fetched.status).toBe(200);
    const got = fetched.body as { fullName: string; nin: string };
    expect(got.fullName).toBe(payload.fullName);
    expect(got.nin).toBe(payload.nin);
  });

  test('error — POST without cooperativeId → 400/422', async () => {
    const payload: Partial<ProducerCreateRequest> = {
      fullName: 'No Coop',
      nin: randNin(),
      phone: randPhone(),
      gpsDomicileLat: 12.0,
      gpsDomicileLon: -1.5,
      primaryCrop: 'coton',
    };
    const created = await core.createProducer(
      payload as ProducerCreateRequest,
    );
    expect(created.status).toBeGreaterThanOrEqual(400);
    expect(created.status).toBeLessThan(500);
  });
});
