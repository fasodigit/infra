// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
// Prometheus Pushgateway client for synthetic monitoring.
// Format: OpenMetrics / text exposition.
//
// Job label : synthetic_monitoring
// Labels    : flow, env, region
// Metrics   :
//   - synthetic_duration_seconds        (gauge)
//   - synthetic_success                 (gauge 0/1)
//   - synthetic_step_duration_seconds   (gauge, per step label)
//   - synthetic_fcp_seconds             (gauge)
//   - synthetic_lcp_seconds             (gauge)
//   - synthetic_tti_seconds             (gauge)
//   - synthetic_http5xx_total           (counter)
//   - synthetic_error_rate              (gauge — 0/1 per flow, aggregated by Prom)

import type { PerfMetrics } from './timing';

const PUSHGATEWAY_URL =
  process.env.PROM_PUSHGATEWAY_URL || 'http://pushgateway:9091';
const ENV = process.env.FASO_ENV || 'prod';
const REGION = process.env.FASO_REGION || 'ouagadougou-1';
const JOB = 'synthetic_monitoring';

export interface StepTiming {
  label: string;
  durationMs: number;
}

export interface SyntheticReport {
  flow: string;
  success: boolean;
  totalDurationMs: number;
  steps: StepTiming[];
  perf?: PerfMetrics;
}

function labelPairs(extra: Record<string, string> = {}): string {
  const all = { flow: '', env: ENV, region: REGION, ...extra };
  return Object.entries(all)
    .filter(([, v]) => v.length > 0)
    .map(([k, v]) => `${k}="${v.replace(/"/g, '\\"')}"`)
    .join(',');
}

function buildPayload(report: SyntheticReport): string {
  const flow = report.flow;
  const success = report.success ? 1 : 0;
  const lines: string[] = [];

  lines.push('# TYPE synthetic_duration_seconds gauge');
  lines.push(
    `synthetic_duration_seconds{${labelPairs({ flow })}} ${(report.totalDurationMs / 1000).toFixed(3)}`,
  );

  lines.push('# TYPE synthetic_success gauge');
  lines.push(`synthetic_success{${labelPairs({ flow })}} ${success}`);

  lines.push('# TYPE synthetic_error_rate gauge');
  lines.push(
    `synthetic_error_rate{${labelPairs({ flow })}} ${report.success ? 0 : 1}`,
  );

  lines.push('# TYPE synthetic_step_duration_seconds gauge');
  for (const step of report.steps) {
    lines.push(
      `synthetic_step_duration_seconds{${labelPairs({ flow, step: step.label })}} ${(step.durationMs / 1000).toFixed(3)}`,
    );
  }

  if (report.perf) {
    lines.push('# TYPE synthetic_fcp_seconds gauge');
    lines.push(
      `synthetic_fcp_seconds{${labelPairs({ flow })}} ${(report.perf.firstContentfulPaintMs / 1000).toFixed(3)}`,
    );
    lines.push('# TYPE synthetic_lcp_seconds gauge');
    lines.push(
      `synthetic_lcp_seconds{${labelPairs({ flow })}} ${(report.perf.largestContentfulPaintMs / 1000).toFixed(3)}`,
    );
    lines.push('# TYPE synthetic_tti_seconds gauge');
    lines.push(
      `synthetic_tti_seconds{${labelPairs({ flow })}} ${(report.perf.timeToInteractiveMs / 1000).toFixed(3)}`,
    );
    lines.push('# TYPE synthetic_http5xx_total counter');
    lines.push(
      `synthetic_http5xx_total{${labelPairs({ flow })}} ${report.perf.http5xxCount}`,
    );
  }

  lines.push('');
  return lines.join('\n');
}

/**
 * Push a single flow report to the Pushgateway.
 * Uses PUT so the group (job=synthetic_monitoring, flow, env, region) is
 * replaced atomically each run, avoiding stale metric accumulation.
 */
export async function pushSyntheticReport(
  report: SyntheticReport,
): Promise<void> {
  const url = `${PUSHGATEWAY_URL}/metrics/job/${JOB}/flow/${encodeURIComponent(report.flow)}/env/${encodeURIComponent(ENV)}/region/${encodeURIComponent(REGION)}`;
  const body = buildPayload(report);

  try {
    const response = await fetch(url, {
      method: 'PUT',
      headers: { 'Content-Type': 'text/plain; version=0.0.4; charset=utf-8' },
      body,
    });
    if (!response.ok) {
      const text = await response.text();
      console.error(
        `[pushgateway] ${response.status} pushing ${report.flow}: ${text}`,
      );
    }
  } catch (err) {
    console.error(`[pushgateway] network error pushing ${report.flow}:`, err);
  }
}
