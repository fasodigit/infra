// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec P0.D — Keto namespaces TERROIR (Tenant / Cooperative / Parcel /
 * HarvestLot) + tuples seedés (Aminata super-admin t_pilot + coopérative
 * pilote).
 *
 * Valide :
 *   - List Tenant tuples → ≥ 2 (admin + gestionnaire pour Aminata sur t_pilot)
 *   - check Tenant:t_pilot#admin@<aminata-uuid> → granted=true
 *   - check Cooperative:<coop-pilot>#parent@Tenant:t_pilot (subject_set) → true
 *   - write tuple test → list confirme présence → delete → liste confirme absence
 *   - check user inexistant → granted=false (pas une erreur, valeur attendue)
 *   - write namespace inconnu → 400
 */
import { test, expect } from '@playwright/test';
import { KetoClient, type RelationTuple } from '../../fixtures/terroir/keto-client';

const AMINATA_UUID =
  process.env.TERROIR_AMINATA_UUID ?? '00000000-0000-4000-8000-000000aminata';
const COOP_PILOT_UUID =
  process.env.TERROIR_COOP_PILOT_UUID ?? '00000000-0000-4000-8000-00000coopilot';
const TENANT_PILOT = 't_pilot';

test.describe('TERROIR P0.D — Keto Tenant namespace', () => {
  let keto: KetoClient;
  let reachable = false;

  test.beforeAll(async () => {
    keto = new KetoClient();
    reachable = await keto.isReachable();
  });

  test.beforeEach(async ({}, testInfo) => {
    if (!reachable) {
      testInfo.skip(
        true,
        'Keto Read :4466 / Write :4467 unreachable — run /cycle-fix first',
      );
    }
  });

  test('list Tenant tuples ≥ 2 (admin + gestionnaire seedés P0.D)', async () => {
    const tuples = await keto.listTuples({ namespace: 'Tenant' });
    expect(tuples.length).toBeGreaterThanOrEqual(2);
    const slugs = tuples.map((t) => t.object);
    expect(slugs).toContain(TENANT_PILOT);
  });

  test('check Tenant:t_pilot#admin@aminata → granted=true', async () => {
    const granted = await keto.checkRelation({
      namespace: 'Tenant',
      object: TENANT_PILOT,
      relation: 'admin',
      subject: { subject_id: AMINATA_UUID },
    });
    expect(granted).toBe(true);
  });

  test('check Cooperative#parent → Tenant:t_pilot (subject_set) → true', async () => {
    const granted = await keto.checkRelation({
      namespace: 'Cooperative',
      object: COOP_PILOT_UUID,
      relation: 'parent',
      subject: {
        subject_set: {
          namespace: 'Tenant',
          object: TENANT_PILOT,
          relation: '',
        },
      },
    });
    expect(granted).toBe(true);
  });

  test('write + read + delete agent_terrain tuple round-trip', async () => {
    const tuple: RelationTuple = {
      namespace: 'Tenant',
      object: 't_e2e_keto',
      relation: 'agent_terrain',
      subject_id: '00000000-0000-4000-8000-0000000e2e001',
    };

    // Pré-cleanup (idempotent).
    await keto.deleteTuple(tuple);

    // 1. Write
    const w = await keto.writeTuple(tuple);
    expect(w.status).toBeGreaterThanOrEqual(200);
    expect(w.status).toBeLessThan(300);

    // 2. Read confirme présence
    const granted = await keto.checkRelation({
      namespace: tuple.namespace,
      object: tuple.object,
      relation: tuple.relation,
      subject: { subject_id: tuple.subject_id! },
    });
    expect(granted).toBe(true);

    // 3. Delete
    const delStatus = await keto.deleteTuple(tuple);
    expect(delStatus).toBeGreaterThanOrEqual(200);
    expect(delStatus).toBeLessThan(300);

    // 4. Read confirme absence
    const grantedAfter = await keto.checkRelation({
      namespace: tuple.namespace,
      object: tuple.object,
      relation: tuple.relation,
      subject: { subject_id: tuple.subject_id! },
    });
    expect(grantedAfter).toBe(false);
  });

  test('check unknown user on Tenant:t_pilot → granted=false (no error)', async () => {
    const granted = await keto.checkRelation({
      namespace: 'Tenant',
      object: TENANT_PILOT,
      relation: 'admin',
      subject: { subject_id: '00000000-0000-4000-8000-0000ghostuser1' },
    });
    expect(granted).toBe(false);
  });

  test('error — write tuple with unknown namespace → 400', async () => {
    const tuple: RelationTuple = {
      namespace: 'NamespaceInconnuNonRegistre',
      object: 'foo',
      relation: 'bar',
      subject_id: '00000000-0000-4000-8000-0000000e2e002',
    };
    const w = await keto.writeTuple(tuple);
    expect(w.status).toBeGreaterThanOrEqual(400);
    expect(w.status).toBeLessThan(500);
  });
});
