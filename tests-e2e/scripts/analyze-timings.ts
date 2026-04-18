#!/usr/bin/env tsx
/**
 * Analyse des timings Playwright - p50/p95/p99
 *
 * Lit reports/results.json (output du json reporter) et produit
 * un resume des timings par test + par requete reseau.
 */

import { readFileSync, existsSync } from 'node:fs';
import { resolve } from 'node:path';

interface PlaywrightTestResult {
  duration?: number;
  title?: string;
  status?: string;
}

interface PlaywrightSpec {
  title?: string;
  tests?: Array<{
    results?: PlaywrightTestResult[];
    title?: string;
  }>;
}

interface PlaywrightSuite {
  title?: string;
  specs?: PlaywrightSpec[];
  suites?: PlaywrightSuite[];
}

interface PlaywrightReport {
  suites?: PlaywrightSuite[];
}

function percentile(values: number[], p: number): number {
  if (values.length === 0) return 0;
  const sorted = [...values].sort((a, b) => a - b);
  const idx = Math.ceil((p / 100) * sorted.length) - 1;
  return sorted[Math.max(0, idx)] ?? 0;
}

function collectDurations(suite: PlaywrightSuite, out: number[]): void {
  for (const spec of suite.specs ?? []) {
    for (const t of spec.tests ?? []) {
      for (const r of t.results ?? []) {
        if (typeof r.duration === 'number' && r.duration > 0) {
          out.push(r.duration);
        }
      }
    }
  }
  for (const s of suite.suites ?? []) {
    collectDurations(s, out);
  }
}

function main(): void {
  const reportPath = resolve(process.cwd(), 'reports/results.json');
  if (!existsSync(reportPath)) {
    process.stderr.write(`[analyze-timings] reports/results.json introuvable. Lancez d'abord: bun run test\n`);
    process.exit(1);
  }

  const raw = readFileSync(reportPath, 'utf-8');
  const report = JSON.parse(raw) as PlaywrightReport;

  const durations: number[] = [];
  for (const s of report.suites ?? []) {
    collectDurations(s, durations);
  }

  if (durations.length === 0) {
    process.stdout.write('[analyze-timings] Aucun test avec duration mesuree.\n');
    return;
  }

  const sum = durations.reduce((a, b) => a + b, 0);
  const avg = sum / durations.length;
  const p50 = percentile(durations, 50);
  const p95 = percentile(durations, 95);
  const p99 = percentile(durations, 99);
  const max = Math.max(...durations);

  process.stdout.write('=== FASO E2E Timings Analysis ===\n');
  process.stdout.write(`Tests mesures : ${durations.length}\n`);
  process.stdout.write(`Total         : ${sum.toFixed(0)} ms\n`);
  process.stdout.write(`Moyenne       : ${avg.toFixed(0)} ms\n`);
  process.stdout.write(`p50           : ${p50.toFixed(0)} ms\n`);
  process.stdout.write(`p95           : ${p95.toFixed(0)} ms\n`);
  process.stdout.write(`p99           : ${p99.toFixed(0)} ms\n`);
  process.stdout.write(`max           : ${max.toFixed(0)} ms\n`);
}

main();
