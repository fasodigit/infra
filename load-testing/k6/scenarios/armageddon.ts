// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
// k6 — Scénarios ARMAGEDDON (gateway Rust souverain) port 8080.
// Scénarios : smoke / load / stress / spike.
// Thresholds : P99 < 10ms, P95 < 5ms, error_rate < 0.001, throughput > 5000 rps.
//
// Invocation :
//   k6 run INFRA/load-testing/k6/scenarios/armageddon.ts
//   K6_SCENARIO=stress k6 run ...
//   ARMAGEDDON_URL=http://gw:8080 AUTH_TOKEN=$(...) k6 run ...

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend, Counter } from 'k6/metrics';
import { buildDevJwt, poulet1KB } from '../lib/helpers.ts';

// -----------------------------------------------------------------------------
// Metrics personnalisées
// -----------------------------------------------------------------------------
const errorRate = new Rate('error_rate');
const throughput = new Counter('throughput_requests');
const healthzLatency = new Trend('healthz_latency_ms', true);
const pouletsLatency = new Trend('poulets_latency_ms', true);
const etatCivilLatency = new Trend('etat_civil_latency_ms', true);

// -----------------------------------------------------------------------------
// Configuration env
// -----------------------------------------------------------------------------
const BASE_URL: string = (__ENV.ARMAGEDDON_URL as string) || 'http://localhost:8080';
const ENVIRONMENT: string = (__ENV.FASO_ENV as string) || 'local';
const SCENARIO: string = (__ENV.K6_SCENARIO as string) || 'smoke';
const AUTH_TOKEN: string = (__ENV.AUTH_TOKEN as string) || buildDevJwt('load-test-user');

// -----------------------------------------------------------------------------
// Scenarios
// -----------------------------------------------------------------------------
const ALL_SCENARIOS = {
  // 10 VUs, 1 min — sanity check
  smoke: {
    executor: 'constant-vus',
    vus: 10,
    duration: '1m',
    tags: { scenario: 'smoke' },
    exec: 'mixedWorkload',
  },
  // 100 VUs, 5 min — SLO baseline
  load: {
    executor: 'ramping-vus',
    startVUs: 0,
    stages: [
      { duration: '30s', target: 100 },
      { duration: '4m',  target: 100 },
      { duration: '30s', target: 0 },
    ],
    gracefulRampDown: '10s',
    tags: { scenario: 'load' },
    exec: 'mixedWorkload',
  },
  // 500 VUs, 10 min — stress
  stress: {
    executor: 'ramping-vus',
    startVUs: 0,
    stages: [
      { duration: '1m', target: 500 },
      { duration: '8m', target: 500 },
      { duration: '1m', target: 0 },
    ],
    gracefulRampDown: '30s',
    tags: { scenario: 'stress' },
    exec: 'mixedWorkload',
  },
  // 0 → 1000 VUs en 30s — spike (validation rate limiter / WAF)
  spike: {
    executor: 'ramping-vus',
    startVUs: 0,
    stages: [
      { duration: '30s', target: 1000 },
      { duration: '1m',  target: 1000 },
      { duration: '30s', target: 0 },
    ],
    gracefulRampDown: '30s',
    tags: { scenario: 'spike' },
    exec: 'mixedWorkload',
  },
};

export const options = {
  scenarios: { [SCENARIO]: ALL_SCENARIOS[SCENARIO as keyof typeof ALL_SCENARIOS] },
  thresholds: {
    http_req_duration: ['p(95)<5', 'p(99)<10'],
    http_req_failed:   ['rate<0.001'],
    error_rate:        ['rate<0.001'],
    throughput_requests: ['rate>5000'],   // >5 000 rps
    healthz_latency_ms:  ['p(99)<3'],
    poulets_latency_ms:  ['p(99)<10'],
    etat_civil_latency_ms: ['p(99)<10'],
    checks: ['rate>0.999'],
  },
  tags: {
    environment: ENVIRONMENT,
    service: 'armageddon',
  },
  summaryTrendStats: ['avg', 'min', 'med', 'p(90)', 'p(95)', 'p(99)', 'max'],
};

// -----------------------------------------------------------------------------
// Distribution du trafic : 50% healthz, 30% POST poulets, 20% GET état-civil
// -----------------------------------------------------------------------------
export function mixedWorkload(): void {
  const roll = Math.random();
  if (roll < 0.5) {
    hitHealthz();
  } else if (roll < 0.8) {
    postPoulet();
  } else {
    getEtatCivilCertificate();
  }
  sleep(0.1);
}

// -----------------------------------------------------------------------------
// Endpoint 1 : GET /healthz (pas d'auth)
// -----------------------------------------------------------------------------
function hitHealthz(): void {
  const res = http.get(`${BASE_URL}/healthz`, {
    tags: { endpoint: 'healthz', environment: ENVIRONMENT, scenario: SCENARIO },
  });
  const ok = check(res, {
    'healthz 200':   (r) => r.status === 200,
    'healthz <10ms': (r) => r.timings.duration < 10,
  });
  errorRate.add(!ok);
  throughput.add(1);
  healthzLatency.add(res.timings.duration);
}

// -----------------------------------------------------------------------------
// Endpoint 2 : POST /api/v1/poulets (payload JSON ~1 KB)
// -----------------------------------------------------------------------------
function postPoulet(): void {
  const body = poulet1KB();
  const res = http.post(`${BASE_URL}/api/v1/poulets`, body, {
    headers: {
      'Content-Type':  'application/json',
      'Accept':        'application/json',
      'Authorization': `Bearer ${AUTH_TOKEN}`,
    },
    tags: { endpoint: 'poulets_post', environment: ENVIRONMENT, scenario: SCENARIO },
  });
  const ok = check(res, {
    'poulets 2xx':       (r) => r.status >= 200 && r.status < 300,
    'poulets has body':  (r) => (r.body?.length ?? 0) > 0,
  });
  errorRate.add(!ok);
  throughput.add(1);
  pouletsLatency.add(res.timings.duration);
}

// -----------------------------------------------------------------------------
// Endpoint 3 : GET /api/v1/etat-civil/certificate (JWT requis)
// -----------------------------------------------------------------------------
function getEtatCivilCertificate(): void {
  const res = http.get(`${BASE_URL}/api/v1/etat-civil/certificate`, {
    headers: {
      'Accept':        'application/json',
      'Authorization': `Bearer ${AUTH_TOKEN}`,
    },
    tags: { endpoint: 'etat_civil_get', environment: ENVIRONMENT, scenario: SCENARIO },
  });
  const ok = check(res, {
    'etat-civil 2xx/401': (r) => r.status === 200 || r.status === 401,
    'etat-civil has body': (r) => (r.body?.length ?? 0) > 0,
  });
  errorRate.add(!ok);
  throughput.add(1);
  etatCivilLatency.add(res.timings.duration);
}

// -----------------------------------------------------------------------------
// Default — utilisé si K6_SCENARIO absent
// -----------------------------------------------------------------------------
export default function (): void {
  mixedWorkload();
}

// -----------------------------------------------------------------------------
// Summary → summary.json (consommé par GH Actions + baseline compare)
// -----------------------------------------------------------------------------
export function handleSummary(data: any) {
  const d = data.metrics.http_req_duration?.values ?? {};
  const f = data.metrics.http_req_failed?.values ?? {};
  const txt = `
=== ARMAGEDDON ${SCENARIO} (${ENVIRONMENT}) ===
  requests : ${data.metrics.http_reqs?.values.count ?? 0}
  rps      : ${(data.metrics.http_reqs?.values.rate ?? 0).toFixed(0)}
  P50      : ${(d.med ?? 0).toFixed(2)} ms
  P95      : ${(d['p(95)'] ?? 0).toFixed(2)} ms
  P99      : ${(d['p(99)'] ?? 0).toFixed(2)} ms
  errors   : ${((f.rate ?? 0) * 100).toFixed(3)}%
`;
  return {
    'summary.json':        JSON.stringify(data, null, 2),
    'summary-armageddon.json': JSON.stringify(data, null, 2),
    stdout: txt,
  };
}
