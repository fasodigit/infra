// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
// k6 smoke test: 10 VUs, 1 min — quick sanity before baseline.

import http from 'k6/http';
import { check, sleep } from 'k6';

export const options = {
  vus: 10,
  duration: '1m',
  thresholds: {
    http_req_duration: ['p(99)<50'],
    http_req_failed:   ['rate<0.01'],
  },
};

const BASE_URL = __ENV.ARMAGEDDON_URL || 'http://localhost:8080';

export default function () {
  const res = http.get(`${BASE_URL}/api/poulets/health`);
  check(res, { 'health 200': (r) => r.status === 200 });
  sleep(0.5);
}
