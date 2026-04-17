// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
// k6 baseline load test for ARMAGEDDON gateway.
// Thresholds: P99 < 10ms, error rate < 0.1%.

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend } from 'k6/metrics';

const errorRate = new Rate('errors');
const routeLatency = new Trend('route_latency_ms', true);

const BASE_URL = __ENV.ARMAGEDDON_URL || 'http://localhost:8080';
const AUTH_TOKEN = __ENV.AUTH_TOKEN || '';

export const options = {
  scenarios: {
    baseline: {
      executor: 'ramping-vus',
      startVUs: 0,
      stages: [
        { duration: '30s', target: 100 },
        { duration: '4m',  target: 100 },
        { duration: '30s', target: 0   },
      ],
      gracefulRampDown: '10s',
    },
  },
  thresholds: {
    http_req_duration: ['p(99)<10'],    // 10 ms P99
    http_req_failed:   ['rate<0.001'],  // 0.1 %
    errors:            ['rate<0.001'],
    checks:            ['rate>0.999'],
  },
  tags: { scenario: 'baseline', service: 'armageddon' },
};

const ROUTES = [
  { path: '/api/poulets/health', weight: 5 },
  { path: '/api/commandes',      weight: 3 },
  { path: '/api/poulets',        weight: 3 },
];

function pickRoute() {
  const total = ROUTES.reduce((s, r) => s + r.weight, 0);
  let roll = Math.random() * total;
  for (const r of ROUTES) { roll -= r.weight; if (roll <= 0) return r.path; }
  return ROUTES[0].path;
}

export default function () {
  const headers: Record<string, string> = { 'Accept': 'application/json' };
  if (AUTH_TOKEN) headers['Authorization'] = `Bearer ${AUTH_TOKEN}`;

  const path = pickRoute();
  const res = http.get(`${BASE_URL}${path}`, { headers, tags: { route: path } });

  const ok = check(res, {
    'status is 2xx/3xx': (r) => r.status < 400,
    'has body':          (r) => (r.body?.length ?? 0) > 0,
  });

  errorRate.add(!ok);
  routeLatency.add(res.timings.duration);

  sleep(0.1);
}

export function handleSummary(data: any) {
  return {
    'summary.json':   JSON.stringify(data, null, 2),
    stdout: textSummary(data),
  };
}

function textSummary(data: any): string {
  const d = data.metrics.http_req_duration.values;
  const f = data.metrics.http_req_failed.values;
  return `
=== ARMAGEDDON baseline ===
  requests : ${data.metrics.http_reqs.values.count}
  P50      : ${d.med.toFixed(2)} ms
  P95      : ${d['p(95)'].toFixed(2)} ms
  P99      : ${d['p(99)'].toFixed(2)} ms
  errors   : ${(f.rate * 100).toFixed(3)}%
`;
}
