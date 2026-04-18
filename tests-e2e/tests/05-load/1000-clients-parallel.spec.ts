import { test } from '@playwright/test';
import fs from 'node:fs';
import { gen1000Clients } from '../../fixtures/actors';
import { quickSignup, postRandomDemand } from '../../fixtures/scenarios';

const clients1000 = gen1000Clients();

const BATCH = Number(process.env.SIM_BATCH ?? 10);
const TX = Number(process.env.SIM_TX_PER_CLIENT ?? 2);

test.describe.configure({ mode: 'parallel' });

for (const [i, c] of clients1000.slice(0, BATCH).entries()) {
  test(`client_${String(i).padStart(4, '0')}: signup + ${TX} tx`, async ({ browser }) => {
    test.setTimeout(180_000);
    const ctx = await browser.newContext();
    const page = await ctx.newPage();
    const samples: number[] = [];
    page.on('requestfinished', (req) => {
      const t = req.timing();
      if (t.responseEnd > 0) samples.push(t.responseEnd - t.requestStart);
    });
    await quickSignup(page, c);
    for (let k = 0; k < TX; k++) await postRandomDemand(page, c);
    await ctx.close();
    fs.mkdirSync('reports', { recursive: true });
    fs.appendFileSync(
      'reports/timings.jsonl',
      JSON.stringify({ client: c.email, samples }) + '\n',
    );
  });
}
