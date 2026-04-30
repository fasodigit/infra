// SPDX-License-Identifier: AGPL-3.0-or-later
// FASO 19-load-p99 — concurrent-user load tests with P50/P95/P99 extraction.
//
// Use the deterministic 500-actor dataset to drive realistic concurrent
// browse / signup / read mixes. Latencies are recorded in-test then
// annotated for the HTML report.
//
// Tempo TraceQL extraction is performed in scripts/p99-by-route.ts after
// the run; this suite focuses on wall-clock measurement.

import { test, expect, request } from '@playwright/test';
import { gen500Actors, pickRandomActor } from '../../fixtures/seed-500';

const GATEWAY = process.env.GATEWAY_URL ?? 'http://localhost:8080';

// Generate the dataset once per test file (idempotent — seeded faker).
const ACTORS = gen500Actors();

function quantile(sorted: number[], q: number): number {
  if (sorted.length === 0) return 0;
  const idx = Math.min(sorted.length - 1, Math.floor(sorted.length * q));
  return sorted[idx]!;
}

function annotate(samples: number[]): { p50: number; p95: number; p99: number; max: number } {
  const sorted = [...samples].sort((a, b) => a - b);
  const p50 = quantile(sorted, 0.5);
  const p95 = quantile(sorted, 0.95);
  const p99 = quantile(sorted, 0.99);
  const max = sorted[sorted.length - 1] ?? 0;
  test.info().annotations.push({ type: 'p50', description: `${p50.toFixed(1)}ms` });
  test.info().annotations.push({ type: 'p95', description: `${p95.toFixed(1)}ms` });
  test.info().annotations.push({ type: 'p99', description: `${p99.toFixed(1)}ms` });
  test.info().annotations.push({ type: 'max', description: `${max.toFixed(1)}ms` });
  test.info().annotations.push({ type: 'samples', description: `${samples.length}` });
  return { p50, p95, p99, max };
}

// ─────────────────────────────────────────────────────────────────────
// Sanity — dataset loads, distribution matches design
// ─────────────────────────────────────────────────────────────────────

test('[@smoke] 500-actor dataset loads with expected distribution', async () => {
  expect(ACTORS).toHaveLength(500);
  const byRole = ACTORS.reduce((acc, a) => {
    acc[a.role] = (acc[a.role] ?? 0) + 1;
    return acc;
  }, {} as Record<string, number>);
  expect(byRole.eleveur).toBe(200);
  expect(byRole.client).toBe(200);
  expect(byRole.admin).toBe(5);
  expect(byRole.veterinaire).toBe(20);
});

// ─────────────────────────────────────────────────────────────────────
// Light load — 10 concurrent gateway probes
// ─────────────────────────────────────────────────────────────────────

test('p99 < 500ms — 10 concurrent /api/health via gateway', async () => {
  // Budget: 500ms covers steady-state gateway p99 (~13ms isolated) PLUS
  // cross-test contention when this suite runs in parallel with others
  // (workers≥2). For an isolated baseline run (workers=1, no other suite),
  // measured p99 stays under 50ms. The headroom catches regressions
  // (e.g., a filter that does sync IO would push p99 over 1s) without
  // flapping under realistic CI parallelism.
  const api = await request.newContext();
  // Warm-up: 5 requests to seed undici keepalive sockets and JIT.
  for (let i = 0; i < 5; i++) await api.get(`${GATEWAY}/api/health`);
  const N = 10;
  const tasks = Array.from({ length: N }, async () => {
    const t0 = performance.now();
    const r = await api.get(`${GATEWAY}/api/health`);
    expect(r.status()).toBe(200);
    return performance.now() - t0;
  });
  const samples = await Promise.all(tasks);
  const { p99 } = annotate(samples);
  expect(p99).toBeLessThan(500);
  await api.dispose();
});

// ─────────────────────────────────────────────────────────────────────
// Medium load — 50 concurrent users, each fires 5 requests
// ─────────────────────────────────────────────────────────────────────

test('p99 < 2000ms — 50 users × 5 reqs (250 total) on /api/health', async () => {
  // Note: Playwright's apiRequestContext uses undici with a default
  // keepalive pool of ~6 sockets. 250 requests serialise through this pool,
  // so the test measures (250/6) sequential round-trips — not 250 truly
  // parallel. Real gateway p99 is ~13ms (see "10 conc /api/health" test).
  // This test asserts the END-TO-END client-perceived budget is reasonable
  // even with client serialisation in dev (host loopback, BFF Next.js).
  const api = await request.newContext();
  const USERS = 50;
  const TX = 5;
  const samples: number[] = [];

  await Promise.all(Array.from({ length: USERS }, async (_, u) => {
    for (let i = 0; i < TX; i++) {
      const t0 = performance.now();
      const r = await api.get(`${GATEWAY}/api/health`);
      samples.push(performance.now() - t0);
      expect(r.status()).toBe(200);
    }
  }));

  const { p99 } = annotate(samples);
  expect(samples.length).toBe(USERS * TX);
  expect(p99).toBeLessThan(2000);
  await api.dispose();
});

// ─────────────────────────────────────────────────────────────────────
// Heavy load — 100 concurrent users, mixed routes
// ─────────────────────────────────────────────────────────────────────

test('p99 < 3500ms — 100 concurrent users mixed routes (clients pickRandom)', async () => {
  // Budget rationale: undici's default agent has ~6 keepalive sockets per
  // origin. 100 parallel `Promise.all` requests serialise through 6 sockets
  // → effective per-socket concurrency = 100/6 ≈ 17. With ~120ms per
  // hop (BFF /api/health is the slowest route here, going through the
  // Next.js dev server), the slowest socket completes around 17×120 =
  // 2040ms, and p99 lands ~2700-3000ms with cross-test contention from
  // other suites running in parallel (workers≥2). 3500ms includes a
  // safety margin for CI noise without becoming a no-op assertion.
  const api = await request.newContext();
  const N = 100;
  const WARMUP = 50;
  const samples: number[] = [];
  const ROUTES = ['/api/health', '/', '/api/auth/health/alive', '/random/foo'];

  // Warm-up phase — sequential, NOT measured. Hydrates JIT, fills the
  // Hikari connection pool on the Java side, and warms keepalive
  // connections in undici. Without this, the FIRST batch's p99 is
  // dominated by cold-start (often 3-4 seconds), not steady-state
  // gateway latency. We chose sequential warm-up because the spec is
  // about p99 under sustained load, not boot-time cold response.
  for (let i = 0; i < WARMUP; i++) {
    const route = ROUTES[i % ROUTES.length]!;
    const r = await api.get(`${GATEWAY}${route}`);
    expect([200, 401, 403, 404]).toContain(r.status());
  }

  await Promise.all(Array.from({ length: N }, async () => {
    const route = ROUTES[Math.floor(Math.random() * ROUTES.length)]!;
    const t0 = performance.now();
    const r = await api.get(`${GATEWAY}${route}`);
    samples.push(performance.now() - t0);
    // Accept 200 (BFF /, /api/health, fallback) or 401/403/404 (auth path no JWT)
    expect([200, 401, 403, 404]).toContain(r.status());
  }));

  const { p99 } = annotate(samples);
  expect(samples.length).toBe(N);
  expect(p99).toBeLessThan(3500);
  await api.dispose();
});

// ─────────────────────────────────────────────────────────────────────
// OPA decision throughput — direct API
// ─────────────────────────────────────────────────────────────────────

test('p99 < 500ms — 100 concurrent OPA decisions (clients/eleveurs mix)', async () => {
  // Real OPA p99 is ~9ms server-side. Client measurement is dominated by
  // undici keepalive serialisation; 500ms is the realistic client budget.
  const api = await request.newContext();
  const N = 100;
  const samples: number[] = [];

  await Promise.all(Array.from({ length: N }, async (_, i) => {
    const role = i % 2 === 0 ? 'eleveur' : 'client';
    const actor = pickRandomActor(ACTORS, role as any);
    const t0 = performance.now();
    const r = await api.post('http://127.0.0.1:8181/v1/data/faso/authz/allow', {
      headers: { 'Content-Type': 'application/json' },
      data: {
        input: {
          path: i % 2 === 0 ? '/api/annonces' : '/api/besoins',
          method: 'POST',
          headers: { authorization: 'Bearer t' },
          jwt_payload: { sub: actor.id, roles: [role] },
        },
      },
    });
    samples.push(performance.now() - t0);
    expect(r.status()).toBe(200);
  }));

  const { p99 } = annotate(samples);
  expect(p99).toBeLessThan(500);
  await api.dispose();
});

// ─────────────────────────────────────────────────────────────────────
// Cache cold vs warm — ARMAGEDDON LB + JWKS cache
// ─────────────────────────────────────────────────────────────────────

test('cold-start p99 vs warm p99 — gateway cache impact < 5x', async () => {
  const api = await request.newContext();
  // First wave — cold (JWKS cache miss possible, LB pool empty)
  const cold: number[] = [];
  for (let i = 0; i < 10; i++) {
    const t0 = performance.now();
    await api.get(`${GATEWAY}/api/health`);
    cold.push(performance.now() - t0);
  }
  cold.sort((a, b) => a - b);
  const coldP99 = cold[cold.length - 1]!;

  // Warm wave — pool full, JWKS cached
  const warm: number[] = [];
  for (let i = 0; i < 50; i++) {
    const t0 = performance.now();
    await api.get(`${GATEWAY}/api/health`);
    warm.push(performance.now() - t0);
  }
  warm.sort((a, b) => a - b);
  const warmP99 = warm[Math.floor(warm.length * 0.99)]!;

  test.info().annotations.push({ type: 'cold p99', description: `${coldP99.toFixed(1)}ms` });
  test.info().annotations.push({ type: 'warm p99', description: `${warmP99.toFixed(1)}ms` });

  // Allow up to 5x slowdown on cold start; otherwise cache is broken
  expect(coldP99 / Math.max(warmP99, 1)).toBeLessThan(5);
  await api.dispose();
});

// ─────────────────────────────────────────────────────────────────────
// 1000 sequential requests — sustained throughput sanity
// ─────────────────────────────────────────────────────────────────────

test('1000 sequential /api/health requests → throughput > 50 req/s', async () => {
  const api = await request.newContext();
  const N = 1000;
  const t0 = performance.now();
  for (let i = 0; i < N; i++) {
    const r = await api.get(`${GATEWAY}/api/health`);
    expect(r.status()).toBe(200);
  }
  const elapsed = (performance.now() - t0) / 1000;
  const rps = N / elapsed;
  test.info().annotations.push({ type: 'rps', description: rps.toFixed(1) });
  test.info().annotations.push({ type: 'elapsed', description: `${elapsed.toFixed(1)}s` });
  expect(rps).toBeGreaterThan(50);
  await api.dispose();
});

// ─────────────────────────────────────────────────────────────────────
// Burst — 200 simultaneous requests (hammer)
// ─────────────────────────────────────────────────────────────────────

test('p99 < 3000ms — 200 simultaneous /api/health (burst)', async () => {
  const api = await request.newContext();
  const N = 200;
  const samples: number[] = [];

  await Promise.all(Array.from({ length: N }, async () => {
    const t0 = performance.now();
    const r = await api.get(`${GATEWAY}/api/health`);
    samples.push(performance.now() - t0);
    expect([200, 503]).toContain(r.status()); // tolerate occasional rate limit
  }));

  const { p99 } = annotate(samples);
  expect(samples.length).toBe(N);
  expect(p99).toBeLessThan(3000);
  await api.dispose();
});
