#!/usr/bin/env bun
/**
 * Orchestrateur de simulation FASO E2E :
 *   - Lance Playwright avec un scope et un projet Chrome
 *   - Classifie les échecs (classify-failures.ts)
 *   - Pause entre chaque itération pour laisser Claude principal dispatcher
 *     les fixes aux agents spécialisés (kaya-rust-implementer, backend, etc.)
 *   - S'arrête dès que 0 échec (GREEN) ou à max-iterations
 *
 * Usage :
 *   bun run scripts/run-simulation.ts --project=chrome-headless-new --max-iterations=5
 */
import { execSync } from 'node:child_process';
import fs from 'node:fs';

const args = process.argv.slice(2);
const project = extractArg(args, '--project', 'chrome-headless-new');
const maxIter = Number(extractArg(args, '--max-iterations', '5'));
const scope = extractArg(
  args,
  '--scope',
  'tests/01-signup tests/02-security tests/03-profile tests/04-business',
);

process.stdout.write(
  `\n🎬 FASO Simulation — project=${project} maxIter=${maxIter}\n`,
);

let passRate = 0;
for (let iter = 1; iter <= maxIter; iter++) {
  const reportDir = `reports/iter-${iter}`;
  fs.mkdirSync(reportDir, { recursive: true });
  process.stdout.write(`\n━━━ Itération ${iter}/${maxIter} ━━━\n`);

  try {
    execSync(
      `bunx playwright test ${scope} --project=${project} --reporter=json --output=${reportDir}/test-results`,
      {
        stdio: 'inherit',
        env: {
          ...process.env,
          PLAYWRIGHT_JSON_OUTPUT_NAME: `${reportDir}/results.json`,
        },
      },
    );
  } catch {
    /* Playwright exits 1 on any failure — on continue */
  }

  execSync(
    `bun run scripts/classify-failures.ts ${reportDir}/results.json ${reportDir}/failures.classified.json`,
  );
  const failures = JSON.parse(
    fs.readFileSync(`${reportDir}/failures.classified.json`, 'utf8'),
  );
  let raw: any = { suites: [] };
  if (fs.existsSync(`${reportDir}/results.json`)) {
    raw = JSON.parse(fs.readFileSync(`${reportDir}/results.json`, 'utf8'));
  }
  const total = countTests(raw);
  const failed = failures.length;
  passRate = total > 0 ? 1 - failed / total : 0;
  process.stdout.write(
    `\n  Total: ${total} · Failed: ${failed} · Pass rate: ${(passRate * 100).toFixed(1)}%\n`,
  );

  if (failures.length === 0) {
    process.stdout.write(`✅ Simulation GREEN à l'itération ${iter}\n`);
    writeFinalReport(iter, total, 1);
    process.exit(0);
  }

  const byAgent = groupBy(failures, (f: any) => f.suggestedAgent);
  process.stdout.write(`\n  Bugs à fixer (par agent cible):\n`);
  for (const [agent, list] of Object.entries(byAgent)) {
    process.stdout.write(`    - ${agent}: ${(list as any[]).length} bugs\n`);
  }
  process.stdout.write(
    `\n  ⏸  Pause orchestrateur: main Claude doit lire ${reportDir}/failures.classified.json et dispatcher aux agents.\n`,
  );
  process.stdout.write(
    `     Appuyer Entrée après fixes déployés (ou Ctrl+C pour sortir).\n`,
  );

  if (!process.stdout.isTTY) {
    process.stdout.write(`     (non-TTY mode — exit pour attendre dispatch manuel)\n`);
    writeFinalReport(iter, total, passRate);
    process.exit(2);
  }
  process.stdin.resume();
  await new Promise((r) => process.stdin.once('data', r));
}

writeFinalReport(maxIter, 0, passRate);

function writeFinalReport(iter: number, total: number, rate: number): void {
  const md = `# Simulation Journal — ${new Date().toISOString()}

Itérations: ${iter} / ${maxIter}
Total tests dernière itération: ${total}
Pass rate final: ${(rate * 100).toFixed(1)}%

Voir reports/iter-*/failures.classified.json pour détails.
`;
  fs.writeFileSync('reports/simulation-journal.md', md);
}

function extractArg(arr: string[], name: string, def: string): string {
  const idx = arr.indexOf(name);
  if (idx >= 0 && arr[idx + 1] !== undefined) return arr[idx + 1] as string;
  const prefix = `${name}=`;
  const eq = arr.find((a) => a.startsWith(prefix));
  if (eq) return eq.slice(prefix.length);
  return def;
}

function countTests(raw: any): number {
  let n = 0;
  function walk(s: any): void {
    for (const spec of s.specs ?? []) {
      for (const t of spec.tests ?? []) {
        n += t.results?.length ?? 0;
      }
    }
    for (const ss of s.suites ?? []) walk(ss);
  }
  for (const s of raw.suites ?? []) walk(s);
  return n;
}

function groupBy<T>(arr: T[], k: (t: T) => string): Record<string, T[]> {
  return arr.reduce((acc: Record<string, T[]>, t) => {
    const key = k(t);
    (acc[key] ??= []).push(t);
    return acc;
  }, {});
}
