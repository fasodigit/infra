import type { FullConfig } from '@playwright/test';
import { MailpitClient } from './mailpit';
import { KratosAdmin } from './kratos';
import { clearSessions } from './session';

export default async function globalSetup(_config: FullConfig): Promise<void> {
  const mailpit = new MailpitClient();
  const kratos = new KratosAdmin();

  const mailpitUp = await mailpit.isReachable();
  const kratosUp = await kratos.isReachable();

  if (mailpitUp) {
    await mailpit.clearAll().catch(() => undefined);
  }

  if (kratosUp) {
    if (process.env.WIPE_IDENTITIES === 'true') {
      // Nuclear option: wipe ALL Kratos identities.
      const deleted = await kratos.wipeAll().catch(() => 0);
      process.stdout.write(`[global-setup] Kratos identities wiped (ALL): ${deleted}\n`);
    } else {
      // Default: scoped wipe of E2E test identities only (@faso-e2e.test domain)
      // so fixture re-runs don't collide with previous registrations.
      const all = await kratos.listIdentities().catch(() => []);
      let deleted = 0;
      for (const id of all) {
        const email = (id.traits as Record<string, unknown> | undefined)?.email;
        if (typeof email === 'string' && email.endsWith('@faso-e2e.test')) {
          if (await kratos.deleteIdentity(id.id)) deleted++;
        }
      }
      process.stdout.write(`[global-setup] Kratos E2E identities wiped: ${deleted}\n`);
    }
  }

  clearSessions();

  process.stdout.write(
    `[global-setup] mailpit=${mailpitUp ? 'UP' : 'DOWN'} kratos=${kratosUp ? 'UP' : 'DOWN'}\n`,
  );
}
