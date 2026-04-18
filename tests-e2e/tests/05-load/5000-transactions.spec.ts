import { test } from '@playwright/test';
import { gen1000Clients } from '../../fixtures/actors';

test.describe.skip('Load - 5000 transactions (run manuel uniquement)', () => {
  // TODO Phase 3: activer ce bloc pour le test de charge full.
  // Pre-requis:
  //   - Stack FASO UP avec KAYA/ARMAGEDDON/BFF/frontend
  //   - Kratos pret a recevoir 1000 inscriptions
  //   - Mailpit en mode ring-buffer (sinon saturation RAM)
  // Commande:
  //   PW_WORKERS=20 bun run test:load

  test('simuler 1000 clients x 5 transactions chacun', async () => {
    const clients = gen1000Clients();
    test.info().annotations.push({
      type: 'load',
      description: `Dataset: ${clients.length} clients, ${clients.length * 5} transactions`,
    });
  });
});
