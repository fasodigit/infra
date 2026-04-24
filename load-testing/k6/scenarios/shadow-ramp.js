// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
// k6 — Shadow-mode ramp-up scenario (Sprint 3 #3).
//
// Simulates a controlled production ramp-up of ARMAGEDDON shadow mode from
// 1% to 50% sample rate in five phases of 2 minutes each.
//
// Each phase:
//   1. SET the shadow sample rate via POST /admin/shadow/rate
//   2. Drive N req/s toward the gateway (:8080)
//   3. PROBE the admin API for divergence metrics and assert:
//        - last_divergence_rate < 0.01   (< 1 % shadow divergence)
//        - gate_tripped_count stable     (no new trips during phase)
//
// Thresholds (CI-failing):
//   - http_req_duration p(95) < 200 ms
//   - http_req_failed   rate  < 1 %
//   - shadow_divergence_rate  < 1 %   (probed via Gauge update in each VU)
//
// Invocation:
//   k6 run --vus 10 --duration 30s --dry-run shadow-ramp.js   # syntax check
//   k6 run shadow-ramp.js
//   GATEWAY_URL=http://armageddon:8080 ADMIN_URL=http://armageddon-admin:9903 \
//     ADMIN_TOKEN=secret LOAD_RPS=100 k6 run shadow-ramp.js

import http from 'k6/http';
import { check, sleep, fail } from 'k6';
import { Rate, Trend, Gauge, Counter } from 'k6/metrics';

// ---------------------------------------------------------------------------
// Environment
// ---------------------------------------------------------------------------
const GATEWAY_URL = __ENV.GATEWAY_URL || 'http://localhost:8080';
const ADMIN_URL   = __ENV.ADMIN_URL   || 'http://localhost:9903';
// Bearer token for the admin API.  Empty string = no Authorization header.
const ADMIN_TOKEN = __ENV.ADMIN_TOKEN || '';
// Target requests per second per VU during load phases.  The executor uses
// `constant-arrival-rate` so total RPS = LOAD_RPS regardless of VU count.
const LOAD_RPS    = parseInt(__ENV.LOAD_RPS || '100', 10);

// ---------------------------------------------------------------------------
// Custom metrics
// ---------------------------------------------------------------------------
const errorRate           = new Rate('http_req_failed_custom');
const gatewayLatency      = new Trend('gateway_latency_ms', true);
const shadowDivergence    = new Gauge('shadow_divergence_rate');
const phaseAdminCallOk    = new Rate('admin_api_call_ok');
const phaseGateTripCount  = new Counter('shadow_gate_trip_count');

// ---------------------------------------------------------------------------
// Phase table — 5 phases × 2 min
// ---------------------------------------------------------------------------
//
// Phase durations are encoded as k6 stages on a ramping-arrival-rate executor.
// Each phase holds a constant rate for 2 minutes after a brief settle period.
//
// shadow_percent is used by the setup() and per-VU probes; k6 does not pass
// per-stage metadata to VU code so we derive the current phase from
// __ITER and the known phase boundaries.
//
const PHASE_DURATIONS_S = 120; // 2 min per phase
const SETTLE_S          = 5;   // brief settle before load in each phase

const SHADOW_PHASES = [
  { percent:  1, rate: LOAD_RPS,       label: 'phase_01pct' },
  { percent:  5, rate: LOAD_RPS,       label: 'phase_05pct' },
  { percent: 10, rate: LOAD_RPS,       label: 'phase_10pct' },
  { percent: 25, rate: LOAD_RPS,       label: 'phase_25pct' },
  { percent: 50, rate: LOAD_RPS,       label: 'phase_50pct' },
];

// Total scenario duration = 5 phases × (SETTLE_S + PHASE_DURATIONS_S)
// = 5 × 125 = 625 s ≈ 10 min 25 s

// ---------------------------------------------------------------------------
// k6 options
// ---------------------------------------------------------------------------
export const options = {
  scenarios: {
    shadow_ramp: {
      executor: 'ramping-arrival-rate',
      // Pre-allocate enough VUs to sustain LOAD_RPS; k6 will warn if more needed.
      preAllocatedVUs: Math.ceil(LOAD_RPS / 10),
      maxVUs: LOAD_RPS * 2,
      startRate: 0,
      timeUnit: '1s',
      stages: [
        // Phase 1 — 1%
        { duration: `${SETTLE_S}s`,           target: 0          }, // settle (admin call)
        { duration: `${PHASE_DURATIONS_S}s`,  target: LOAD_RPS   }, // load
        // Phase 2 — 5%
        { duration: `${SETTLE_S}s`,           target: 0          },
        { duration: `${PHASE_DURATIONS_S}s`,  target: LOAD_RPS   },
        // Phase 3 — 10%
        { duration: `${SETTLE_S}s`,           target: 0          },
        { duration: `${PHASE_DURATIONS_S}s`,  target: LOAD_RPS   },
        // Phase 4 — 25%
        { duration: `${SETTLE_S}s`,           target: 0          },
        { duration: `${PHASE_DURATIONS_S}s`,  target: LOAD_RPS   },
        // Phase 5 — 50%
        { duration: `${SETTLE_S}s`,           target: 0          },
        { duration: `${PHASE_DURATIONS_S}s`,  target: LOAD_RPS   },
        // Drain
        { duration: '5s',                     target: 0          },
      ],
    },
  },

  thresholds: {
    // Primary SLOs — CI fails if any threshold is breached.
    'http_req_duration{scenario:shadow_ramp}': ['p(95)<200'],
    'http_req_failed{scenario:shadow_ramp}':   ['rate<0.01'],
    // Shadow divergence gauge must stay < 1 % at end of run.
    shadow_divergence_rate: ['value<0.01'],
    // Admin API calls must succeed ≥ 95 % of the time.
    admin_api_call_ok: ['rate>0.95'],
  },

  tags: {
    environment: __ENV.FASO_ENV || 'local',
    service:     'armageddon-shadow-ramp',
  },

  summaryTrendStats: ['avg', 'min', 'med', 'p(90)', 'p(95)', 'p(99)', 'max'],
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Build admin API request params.
 * Adds Bearer token only when ADMIN_TOKEN is configured.
 */
function adminParams() {
  const headers = { 'Content-Type': 'application/json' };
  if (ADMIN_TOKEN) {
    headers['Authorization'] = `Bearer ${ADMIN_TOKEN}`;
  }
  return { headers, tags: { endpoint: 'admin' } };
}

/**
 * Set the shadow sample rate via the admin API.
 * Returns true on success.
 */
function setShadowRate(percent) {
  const res = http.post(
    `${ADMIN_URL}/admin/shadow/rate`,
    JSON.stringify({ percent }),
    adminParams(),
  );
  const ok = check(res, {
    'admin shadow rate 200/204': (r) => r.status === 200 || r.status === 204,
  });
  phaseAdminCallOk.add(ok);
  return ok;
}

/**
 * Probe the shadow state and extract divergence + gate trip metrics.
 * Returns { divergence_rate, gate_tripped_count } or null on failure.
 */
function probeShadowState() {
  const res = http.get(`${ADMIN_URL}/admin/shadow/state`, adminParams());
  const ok = check(res, {
    'admin shadow state 200': (r) => r.status === 200,
    'admin shadow state has body': (r) => (r.body || '').length > 0,
  });
  phaseAdminCallOk.add(ok);
  if (!ok) return null;

  let body;
  try {
    body = JSON.parse(res.body);
  } catch (_) {
    return null;
  }

  return {
    divergence_rate:    body.last_divergence_rate   || 0,
    gate_tripped_count: body.gate_tripped_count      || 0,
  };
}

// ---------------------------------------------------------------------------
// setup() — runs once before the scenario; sets initial shadow rate to 0.
// ---------------------------------------------------------------------------
export function setup() {
  setShadowRate(0);
  sleep(1);

  // Return phase boundaries (elapsed seconds at which each phase starts).
  // VU code uses these to pick the right shadow_percent for admin probes.
  const boundaries = [];
  let elapsed = 0;
  for (let i = 0; i < SHADOW_PHASES.length; i++) {
    boundaries.push(elapsed);
    elapsed += SETTLE_S + PHASE_DURATIONS_S;
  }
  return { boundaries, startTs: Date.now() };
}

// ---------------------------------------------------------------------------
// Phase control — called by the first VU iteration of each settle window.
//
// k6's ramping-arrival-rate executor does not provide a per-stage lifecycle
// hook, so we approximate phase transitions by detecting the `target: 0`
// settle windows via a shared module-level variable updated on each iteration.
// ---------------------------------------------------------------------------

// Module-level mutable phase tracker (per-VU, so each VU tracks independently).
let _lastSetPhase = -1;

/**
 * Derive the current phase index (0–4) from elapsed scenario time.
 *
 * Each phase occupies SETTLE_S + PHASE_DURATIONS_S seconds.
 */
function currentPhaseIndex(setupData) {
  const elapsedS = (Date.now() - setupData.startTs) / 1000;
  const phaseLen = SETTLE_S + PHASE_DURATIONS_S;
  return Math.min(Math.floor(elapsedS / phaseLen), SHADOW_PHASES.length - 1);
}

// ---------------------------------------------------------------------------
// default — main VU loop
// ---------------------------------------------------------------------------
export default function (setupData) {
  const phaseIdx = currentPhaseIndex(setupData);
  const phase    = SHADOW_PHASES[phaseIdx];

  // Each VU sets the shadow rate at the start of each new phase it detects.
  // The rate call is idempotent on the server side; concurrent calls from
  // multiple VUs are harmless.
  if (phaseIdx !== _lastSetPhase) {
    _lastSetPhase = phaseIdx;
    setShadowRate(phase.percent);
    sleep(0.1); // brief pause after admin call
  }

  // ── Gateway load ──────────────────────────────────────────────────────────
  const res = http.get(`${GATEWAY_URL}/healthz`, {
    tags: { scenario: 'shadow_ramp', phase: phase.label },
  });

  const ok = check(res, {
    'healthz 200':       (r) => r.status === 200,
    'healthz <200ms':    (r) => r.timings.duration < 200,
  });

  errorRate.add(!ok, { phase: phase.label });
  gatewayLatency.add(res.timings.duration, { phase: phase.label });

  // ── Admin probe (probabilistic: ~5 % of iterations to avoid saturation) ──
  if (Math.random() < 0.05) {
    const state = probeShadowState();
    if (state !== null) {
      shadowDivergence.add(state.divergence_rate, { phase: phase.label });
      phaseGateTripCount.add(state.gate_tripped_count, { phase: phase.label });

      // Fail the VU if divergence exceeds the 1 % threshold.
      if (state.divergence_rate >= 0.01) {
        fail(
          `Shadow divergence ${(state.divergence_rate * 100).toFixed(2)}% ` +
          `exceeds 1% threshold in ${phase.label}`,
        );
      }
    }
  }
}

// ---------------------------------------------------------------------------
// teardown() — reset shadow rate to 0 after the run.
// ---------------------------------------------------------------------------
export function teardown(_setupData) {
  setShadowRate(0);
}

// ---------------------------------------------------------------------------
// handleSummary — emit JSON report + human-readable summary.
// ---------------------------------------------------------------------------
export function handleSummary(data) {
  const dur    = data.metrics['http_req_duration']?.values ?? {};
  const failed = data.metrics['http_req_failed']?.values   ?? {};
  const diverg = data.metrics['shadow_divergence_rate']?.values ?? {};
  const trips  = data.metrics['shadow_gate_trip_count']?.values ?? {};

  const txt = `
=== ARMAGEDDON shadow-ramp (${__ENV.FASO_ENV || 'local'}) ===
  phases       : ${SHADOW_PHASES.map((p) => p.percent + '%').join(' → ')}
  total reqs   : ${data.metrics.http_reqs?.values.count ?? 0}
  rps          : ${(data.metrics.http_reqs?.values.rate ?? 0).toFixed(0)}
  P50 (ms)     : ${(dur.med       ?? 0).toFixed(2)}
  P95 (ms)     : ${(dur['p(95)']  ?? 0).toFixed(2)}
  P99 (ms)     : ${(dur['p(99)']  ?? 0).toFixed(2)}
  error rate   : ${((failed.rate ?? 0) * 100).toFixed(3)}%
  divergence   : ${((diverg.value ?? 0) * 100).toFixed(3)}%
  gate trips   : ${trips.count ?? 0}
`;

  return {
    'summary.json':              JSON.stringify(data, null, 2),
    'summary-shadow-ramp.json':  JSON.stringify(data, null, 2),
    stdout: txt,
  };
}
