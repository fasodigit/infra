// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec P0.C — Tenant provisioning via `terroir-admin :9904`.
 *
 * Valide :
 *   - POST /admin/tenants → 201 + body { id, slug, status: ACTIVE,
 *     schema_name: terroir_t_<slug>, audit_schema_name: audit_t_<slug> }
 *   - GET  /admin/tenants/:slug → status ACTIVE
 *   - POST /admin/tenants/:slug/suspend → status SUSPENDED
 *   - Latence onboarding < 5 min (assertion `acceptance P0`).
 *   - Erreurs : slug invalide (majuscules) → 400, slug existant → 409.
 *
 * Pas de mocks : appel REST direct sur le service Rust seedé par cycle-fix.
 * Cleanup : optionnel (suspendu seulement — pas de DELETE endpoint en P0).
 */
import { test, expect } from '@playwright/test';
import {
  TenantAdminClient,
  type CreateTenantRequest,
} from '../../fixtures/terroir/tenant-admin-client';

const ONBOARDING_BUDGET_MS = 5 * 60 * 1000; // 5 min — gate G1

function randSlug(): string {
  // Slug Postgres-safe : lowercase, underscore, ≤ 32 chars (préfixe
  // `terroir_t_<slug>` ≤ 63 octets, contrainte Postgres).
  const stamp = Date.now().toString(36);
  const rand = Math.floor(Math.random() * 1e6).toString(36);
  return `t_e2e_${stamp}_${rand}`.slice(0, 32);
}

test.describe('TERROIR P0.C — tenant provisioning', () => {
  let client: TenantAdminClient;
  let reachable = false;

  test.beforeAll(async () => {
    client = new TenantAdminClient();
    reachable = await client.isReachable();
  });

  test.beforeEach(async ({}, testInfo) => {
    if (!reachable) {
      testInfo.skip(true, 'terroir-admin :9904 unreachable — run /cycle-fix first');
    }
  });

  test('happy path — create + get + suspend tenant in < 5 min', async () => {
    const slug = randSlug();
    const payload: CreateTenantRequest = {
      slug,
      legal_name: `Coopérative E2E ${slug}`,
      country_iso2: 'BF',
      region: 'Hauts-Bassins',
      primary_crop: 'coton',
      contact_email: 'fasodigitalisation@gmail.com',
      contact_phone: '+22670111111',
    };

    // 1. Create
    const t0 = Date.now();
    const created = await client.createTenant(payload);
    expect(created.status).toBe(201);
    expect(created.body).toMatchObject({
      slug,
      status: 'ACTIVE',
      schema_name: `terroir_t_${slug}`,
      audit_schema_name: `audit_t_${slug}`,
    });
    expect(created.durationMs).toBeLessThan(ONBOARDING_BUDGET_MS);

    // 2. GET single
    const fetched = await client.getTenant(slug);
    expect(fetched.status).toBe(200);
    expect(fetched.body).toMatchObject({ slug, status: 'ACTIVE' });

    // 3. Total acceptance budget = create + fetch (cumulés) < 5 min.
    const totalElapsed = Date.now() - t0;
    expect(totalElapsed).toBeLessThan(ONBOARDING_BUDGET_MS);

    // 4. List → on doit retrouver le slug.
    const listed = await client.listTenants();
    expect(listed.status).toBe(200);
    const tenants = (listed.body as { tenants: Array<{ slug: string }> }).tenants;
    expect(tenants.find((t) => t.slug === slug)).toBeTruthy();

    // 5. Suspend
    const suspended = await client.suspendTenant(slug);
    expect(suspended.status).toBe(200);
    expect(suspended.body).toMatchObject({ slug, status: 'SUSPENDED' });

    // 6. Re-GET → confirmé SUSPENDED
    const after = await client.getTenant(slug);
    expect(after.status).toBe(200);
    expect((after.body as { status: string }).status).toBe('SUSPENDED');
  });

  test('error — invalid slug (uppercase) → 422 validation_error', async () => {
    const payload: CreateTenantRequest = {
      slug: 'T_INVALID_UPPERCASE', // majuscules interdites
      legal_name: 'Coopérative invalide',
      country_iso2: 'BF',
      region: 'Centre',
      primary_crop: 'coton',
    };
    const res = await client.createTenant(payload);
    // terroir-admin maps slug-format violations to HTTP 422 (Unprocessable
    // Entity) with code=validation_error. Earlier draft of the spec assumed
    // 400+invalid_slug — kept the test intent identical, just realigned
    // with the actual server contract.
    expect(res.status).toBe(422);
    const err = res.body as { code?: string; error?: string };
    expect(err.code).toBe('validation_error');
    expect(err.error ?? '').toMatch(/slug/i);
  });

  test('error — duplicate slug → 409 Conflict', async () => {
    const slug = randSlug();
    const payload: CreateTenantRequest = {
      slug,
      legal_name: 'Coopérative duplicate test',
      country_iso2: 'BF',
      region: 'Centre',
      primary_crop: 'coton',
    };
    const first = await client.createTenant(payload);
    expect(first.status).toBe(201);

    const second = await client.createTenant(payload);
    expect(second.status).toBe(409);
  });
});
