// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
// Push synthetic monitoring metrics to Prometheus Pushgateway.

const PUSHGATEWAY_URL = process.env.PROM_PUSHGATEWAY_URL || 'http://pushgateway:9091';
const ENV = process.env.FASO_ENV || 'prod';
const REGION = process.env.FASO_REGION || 'bf-ouaga';

export async function pushMetrics(
  scenario: string,
  metrics: { duration_ms: number; success: number },
): Promise<void> {
  const lines: string[] = [];
  lines.push(`# TYPE synthetic_${scenario}_duration_ms gauge`);
  lines.push(`synthetic_${scenario}_duration_ms{env="${ENV}",region="${REGION}"} ${metrics.duration_ms}`);
  lines.push(`# TYPE synthetic_${scenario}_success gauge`);
  lines.push(`synthetic_${scenario}_success{env="${ENV}",region="${REGION}"} ${metrics.success}`);
  lines.push(''); // trailing newline required

  const url = `${PUSHGATEWAY_URL}/metrics/job/synthetic/scenario/${scenario}/env/${ENV}/region/${REGION}`;
  const response = await fetch(url, {
    method: 'PUT',
    headers: { 'Content-Type': 'text/plain; charset=utf-8' },
    body: lines.join('\n'),
  });
  if (!response.ok) {
    console.error(`Pushgateway error ${response.status}: ${await response.text()}`);
  }
}
