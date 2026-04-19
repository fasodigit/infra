// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
// k6 + xk6-redis — Scénario RESP3 ciblant KAYA port 6380.
// Distribution commandes : SET 70% / GET 15% / INCR 10% / HSET 3% / ZADD 2%.
// Thresholds : P99 < 1 ms, throughput > 100 000 ops/s.
//
// Prérequis : k6 build via xk6 :
//   xk6 build --with github.com/grafana/xk6-redis
//
// Invocation :
//   KAYA_URL=redis://localhost:6380 ./k6 run scenarios/kaya-resp3.ts

// @ts-ignore — module fourni par xk6-redis en runtime
import redis from 'k6/x/redis';
import { check } from 'k6';
import { Trend, Counter, Rate } from 'k6/metrics';
import { kayaKey, kayaSmallValue } from '../lib/helpers.ts';

// -----------------------------------------------------------------------------
// Metrics personnalisées
// -----------------------------------------------------------------------------
const opsCounter = new Counter('kaya_ops_total');
const opsLatency = new Trend('kaya_op_latency_ms', true);
const opErrors = new Rate('kaya_op_errors');

// -----------------------------------------------------------------------------
// Connexion KAYA (RESP3)
// -----------------------------------------------------------------------------
const KAYA_URL: string = (__ENV.KAYA_URL as string) || 'redis://localhost:6380';
const ENVIRONMENT: string = (__ENV.FASO_ENV as string) || 'local';

const client = new redis.Client({
  addrs: [KAYA_URL.replace(/^redis:\/\//, '')],
  protocol: 3,   // RESP3
});

// -----------------------------------------------------------------------------
// Options k6
// -----------------------------------------------------------------------------
export const options = {
  scenarios: {
    kaya_resp3: {
      executor: 'ramping-vus',
      startVUs: 0,
      stages: [
        { duration: '30s', target: 200 },
        { duration: '3m',  target: 200 },
        { duration: '30s', target: 0 },
      ],
      tags: { scenario: 'kaya_resp3' },
    },
  },
  thresholds: {
    kaya_op_latency_ms: ['p(99)<1'],         // P99 < 1 ms
    kaya_ops_total:     ['rate>100000'],      // > 100k ops/s
    kaya_op_errors:     ['rate<0.001'],
    checks:             ['rate>0.999'],
  },
  tags: {
    environment: ENVIRONMENT,
    service: 'kaya',
  },
  summaryTrendStats: ['avg', 'min', 'med', 'p(90)', 'p(95)', 'p(99)', 'max'],
};

// -----------------------------------------------------------------------------
// Sélection de commande pondérée (70/15/10/3/2 %)
// -----------------------------------------------------------------------------
function pickCommand(): 'SET' | 'GET' | 'INCR' | 'HSET' | 'ZADD' {
  const r = Math.random() * 100;
  if (r < 70) return 'SET';
  if (r < 85) return 'GET';
  if (r < 95) return 'INCR';
  if (r < 98) return 'HSET';
  return 'ZADD';
}

// -----------------------------------------------------------------------------
// Boucle principale : 1 commande RESP3 par itération
// -----------------------------------------------------------------------------
export default async function (): Promise<void> {
  const cmd = pickCommand();
  const start = Date.now();
  let ok = true;

  try {
    switch (cmd) {
      case 'SET':
        await client.set(kayaKey('poulet'), kayaSmallValue(), 60);
        break;
      case 'GET':
        await client.get(kayaKey('poulet'));
        break;
      case 'INCR':
        await client.incr(kayaKey('counter'));
        break;
      case 'HSET':
        await client.hset(kayaKey('hash'), 'field', kayaSmallValue());
        break;
      case 'ZADD':
        await client.zadd(kayaKey('leaderboard'), Math.random() * 1000, `member-${Math.floor(Math.random() * 10000)}`);
        break;
    }
  } catch (e) {
    ok = false;
  }

  const elapsed = Date.now() - start;
  opsCounter.add(1, { cmd });
  opsLatency.add(elapsed, { cmd });
  opErrors.add(!ok, { cmd });

  check(ok, { [`kaya ${cmd} ok`]: (v) => v === true });
}

// -----------------------------------------------------------------------------
// Summary
// -----------------------------------------------------------------------------
export function handleSummary(data: any) {
  const l = data.metrics.kaya_op_latency_ms?.values ?? {};
  const c = data.metrics.kaya_ops_total?.values ?? {};
  const txt = `
=== KAYA RESP3 (${ENVIRONMENT}) ===
  ops       : ${c.count ?? 0}
  ops/s     : ${(c.rate ?? 0).toFixed(0)}
  P50 (ms)  : ${(l.med ?? 0).toFixed(3)}
  P95 (ms)  : ${(l['p(95)'] ?? 0).toFixed(3)}
  P99 (ms)  : ${(l['p(99)'] ?? 0).toFixed(3)}
`;
  return {
    'summary.json':      JSON.stringify(data, null, 2),
    'summary-kaya.json': JSON.stringify(data, null, 2),
    stdout: txt,
  };
}
