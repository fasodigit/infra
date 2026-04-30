// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec P0.B — Vault Transit `terroir-pii-master` encrypt/decrypt round-trip.
 *
 * Valide :
 *   - encrypt(plaintext, context) → ciphertext préfixé `vault:v1:`.
 *   - decrypt(ciphertext, same context) → plaintext recovered.
 *   - decrypt avec context différent → 400 (key derivation mismatch).
 *   - /v1/sys/health → 200 (init + unsealed).
 *   - /v1/transit/keys/terroir-pii-master → derived=true,
 *     auto_rotate_period 2160h (90j), latest_version ≥ 1.
 *   - Erreurs : encrypt sans context → 400 ; decrypt ciphertext altéré → 400.
 */
import { test, expect } from '@playwright/test';
import { VaultTransitClient } from '../../fixtures/terroir/vault-transit-client';

test.describe('TERROIR P0.B — Vault Transit terroir-pii-master', () => {
  let vault: VaultTransitClient;
  let reachable = false;
  let hasToken = false;

  test.beforeAll(async () => {
    vault = new VaultTransitClient();
    reachable = await vault.isReachable();
    hasToken = !!process.env.VAULT_TOKEN;
  });

  test.beforeEach(async ({}, testInfo) => {
    if (!reachable) {
      testInfo.skip(true, 'Vault :8200 unreachable — run /cycle-fix first');
    }
    if (!hasToken) {
      testInfo.skip(
        true,
        'VAULT_TOKEN absent — export $(jq -r .root_token ~/.faso-vault-keys.json)',
      );
    }
  });

  test('happy path — encrypt+decrypt nin & phone with tenant context', async () => {
    const pii = {
      nin: 'BF-1234567890',
      phone: '+22670111111',
    };
    const tenantSlug = 't_pilot';

    for (const [field, plaintext] of Object.entries(pii)) {
      const context = `tenant=${tenantSlug}|field=${field}`;

      // 1. Encrypt
      const enc = await vault.encrypt({ plaintext, context });
      expect(enc.ciphertext).toMatch(/^vault:v\d+:/);
      expect(enc.key_version).toBeGreaterThanOrEqual(1);

      // 2. Decrypt with same context → recovers plaintext
      const recovered = await vault.decrypt(enc.ciphertext, context);
      expect(recovered).toBe(plaintext);

      // 3. Decrypt with different context → cipher auth fails
      const wrong = await vault.decryptRaw({
        ciphertext: enc.ciphertext,
        context: Buffer.from(`tenant=t_other|field=${field}`).toString('base64'),
      });
      expect(wrong.status).toBeGreaterThanOrEqual(400);
      const errBody = wrong.body as { errors?: string[] };
      expect(errBody.errors?.join(' ') ?? '').toMatch(
        /authentication|cipher|message|decrypt/i,
      );
    }
  });

  test('vault key info — derived=true, auto_rotate_period=90 days', async () => {
    const info = await vault.getKeyInfo();
    expect(info.name).toBe('terroir-pii-master');
    expect(info.derived).toBe(true);
    expect(info.type).toMatch(/aes256-gcm96/i);
    expect(info.latest_version).toBeGreaterThanOrEqual(1);
    expect(info.min_decryption_version).toBeGreaterThanOrEqual(1);
    // Vault returns auto_rotate_period as nanoseconds (Go duration) when
    // serialised over /v1/transit/keys/<key>. 2160h == 90d == 7_776_000s
    // == 7_776_000_000_000_000ns. Newer Vault releases (≥1.18) emit the
    // value as integer seconds (7776000) — accept either.
    const auto = info.auto_rotate_period as unknown;
    const SECONDS_90D = 90 * 24 * 3600;
    if (typeof auto === 'string') {
      // Older Vault emits "2160h" or "7776000s".
      expect(auto).toMatch(/^2160h$|^7776000s?$/);
    } else {
      // Numeric (seconds or ns).
      const n = Number(auto);
      expect([SECONDS_90D, SECONDS_90D * 1_000_000_000]).toContain(n);
    }
  });

  test('vault sys/health → 200 + initialized + unsealed', async () => {
    const h = await vault.health();
    expect(h.status).toBe(200);
    expect(h.initialized).toBe(true);
    expect(h.sealed).toBe(false);
  });

  test('error — encrypt without context (derived=true) → 400', async () => {
    const res = await vault.encryptRaw({
      plaintext: Buffer.from('secret-no-ctx').toString('base64'),
      // context omis volontairement
    });
    expect(res.status).toBeGreaterThanOrEqual(400);
    const errBody = res.body as { errors?: string[] };
    expect(errBody.errors?.join(' ') ?? '').toMatch(/context|derived/i);
  });

  test('error — decrypt with tampered ciphertext → 400 auth failed', async () => {
    const context = 'tenant=t_pilot|field=nin';
    const enc = await vault.encrypt({
      plaintext: 'BF-9999999999',
      context,
    });

    // Altère le dernier caractère du ciphertext.
    const tampered =
      enc.ciphertext.slice(0, -1) +
      (enc.ciphertext.slice(-1) === 'A' ? 'B' : 'A');

    const res = await vault.decryptRaw({
      ciphertext: tampered,
      context: Buffer.from(context).toString('base64'),
    });
    expect(res.status).toBeGreaterThanOrEqual(400);
    const errBody = res.body as { errors?: string[] };
    expect(errBody.errors?.join(' ') ?? '').toMatch(
      /authentication|cipher|invalid/i,
    );
  });
});
