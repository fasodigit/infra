#!/usr/bin/env bun
// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
// Seed bootstrap — 500 actors + cascading offers/demands/orders.
//
// Usage:
//   bun run scripts/seed-data.ts         # idempotent: skip if snapshot exists
//   bun run scripts/seed-data.ts --reset # wipe + re-seed from scratch
//
// Pre-requisites:
//   - Kratos public + admin reachable (4433/4434 via gateway 8080 if proxied)
//   - poulets-api reachable at GATEWAY_URL or via /api/* routes
//
// Output:
//   tests-e2e/reports/seed-state.json — counts + per-role distribution.

import fs from 'node:fs';
import path from 'node:path';
import { request } from '@playwright/test';
import {
  gen500Actors, genOffers, genDemands, genOrders, snapshot,
  type SeedSnapshot,
} from '../fixtures/seed-500';
import { KratosAdmin } from '../fixtures/kratos';

const GATEWAY     = process.env.GATEWAY_URL     ?? 'http://localhost:8080';
const KRATOS_PUB  = process.env.KRATOS_PUB_URL  ?? 'http://localhost:4433';
const KRATOS_ADM  = process.env.KRATOS_ADM_URL  ?? 'http://localhost:4434';
const REPORT      = path.resolve(__dirname, '..', 'reports', 'seed-state.json');
const RESET       = process.argv.includes('--reset');
const DRY_RUN     = process.argv.includes('--dry-run');

async function main(): Promise<void> {
  process.stdout.write(`\n🌱 FASO seed-data\n`);
  process.stdout.write(`   gateway: ${GATEWAY}\n`);
  process.stdout.write(`   kratos:  ${KRATOS_PUB} (admin ${KRATOS_ADM})\n`);
  process.stdout.write(`   reset:   ${RESET}\n`);
  process.stdout.write(`   dryRun:  ${DRY_RUN}\n\n`);

  // ── 1. Generate the deterministic dataset ────────────────────────────
  const actors  = gen500Actors();
  const offers  = genOffers(actors);
  const demands = genDemands(actors);
  const orders  = genOrders(offers, demands, actors);
  const snap    = snapshot(actors, offers, demands, orders);

  process.stdout.write(`[seed] generated:\n`);
  process.stdout.write(`         actors:  ${snap.counts.actors}  ${JSON.stringify(snap.byRole)}\n`);
  process.stdout.write(`         offers:  ${snap.counts.offers}\n`);
  process.stdout.write(`         demands: ${snap.counts.demands}\n`);
  process.stdout.write(`         orders:  ${snap.counts.orders}\n\n`);

  // ── 2. Idempotence check ─────────────────────────────────────────────
  if (!RESET && fs.existsSync(REPORT)) {
    const prev: SeedSnapshot = JSON.parse(fs.readFileSync(REPORT, 'utf8'));
    if (prev.counts.actors === snap.counts.actors) {
      process.stdout.write(`[seed] snapshot exists at ${REPORT} (actors=${prev.counts.actors}); skipping. Use --reset to force.\n`);
      return;
    }
  }

  if (DRY_RUN) {
    process.stdout.write(`[seed] dry-run: skipping all network calls.\n`);
    writeReport(snap);
    return;
  }

  // ── 3. Kratos: wipe E2E test identities ──────────────────────────────
  const kratos = new KratosAdmin(KRATOS_ADM, KRATOS_PUB);
  if (!(await kratos.isReachable())) {
    throw new Error(`Kratos public not reachable at ${KRATOS_PUB}`);
  }

  if (RESET) {
    process.stdout.write(`[seed] wiping E2E identities...\n`);
    const all = await kratos.listIdentities();
    let wiped = 0;
    for (const id of all) {
      const email = (id.traits as Record<string, unknown> | undefined)?.email;
      if (typeof email === 'string' && email.endsWith('@faso-e2e.test')) {
        if (await kratos.deleteIdentity(id.id)) wiped++;
      }
    }
    process.stdout.write(`[seed] wiped ${wiped} E2E identities.\n`);
  }

  // ── 4. Bulk-create 500 identities via Kratos admin (parallel batches) ─
  const api = await request.newContext();
  let created = 0;
  let skipped = 0;
  const batches = chunk(actors, 25);

  for (const [batchIdx, batch] of batches.entries()) {
    process.stdout.write(`\r[seed] batch ${batchIdx + 1}/${batches.length}  created=${created} skipped=${skipped}`);
    const results = await Promise.allSettled(batch.map(async (a) => {
      const res = await api.post(`${KRATOS_ADM}/admin/identities`, {
        data: {
          schema_id: 'default',
          traits: {
            email: a.email,
            name: { first: a.firstName, last: a.lastName },
            phone: a.phone,
            role:  a.role,
          },
          credentials: {
            password: { config: { password: a.password } },
          },
        },
      });
      if (res.ok()) return 'created';
      if (res.status() === 409) return 'skipped';
      throw new Error(`unexpected ${res.status()} from Kratos`);
    }));
    for (const r of results) {
      if (r.status === 'fulfilled') {
        if (r.value === 'created') created++; else skipped++;
      } else {
        process.stderr.write(`\n[seed] error: ${r.reason}\n`);
      }
    }
  }
  process.stdout.write(`\n[seed] kratos identities: created=${created} skipped=${skipped}\n`);

  // ── 5. Cascading artifacts via gateway → poulets-api ─────────────────
  // NOTE: requires gateway routes /api/annonces, /api/besoins, /api/commandes
  // and a service-token header that bypasses normal user auth (for seed-only).
  // In dev we shell out to a SQL importer to avoid GraphQL auth overhead.
  // This step is a STUB — wire up your specific service when implementing.
  process.stdout.write(`[seed] offers/demands/orders cascade is STUB-MODE (snapshot file only).\n`);
  process.stdout.write(`       Implement HTTP/GraphQL bulk import per your service contract.\n`);

  await api.dispose();

  // ── 6. Persist snapshot ──────────────────────────────────────────────
  writeReport(snap);
  process.stdout.write(`\n[seed] ✅ snapshot written to ${REPORT}\n`);
}

function chunk<T>(arr: T[], size: number): T[][] {
  const out: T[][] = [];
  for (let i = 0; i < arr.length; i += size) out.push(arr.slice(i, i + size));
  return out;
}

function writeReport(snap: SeedSnapshot): void {
  fs.mkdirSync(path.dirname(REPORT), { recursive: true });
  fs.writeFileSync(REPORT, JSON.stringify(snap, null, 2));
}

main().catch((e) => {
  process.stderr.write(`[seed] FATAL: ${e}\n`);
  process.exit(1);
});
