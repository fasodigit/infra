// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Spec P0.F — terroir-ussd-simulator :1080 round-trip.
 *
 * Valide :
 *   - Hub2 producer-signup : 5 steps → END Inscription validée.
 *   - OTP 8 digits capturé via /admin/last-sms (regex `\b(\d{8})\b`).
 *   - clearAll() supprime SMS et sessions.
 *   - Erreur : mauvais OTP → END Code OTP invalide ou expire.
 *   - Cross-provider : Twilio /sms/send → /admin/last-sms indexé MSISDN.
 */
import { test, expect } from '@playwright/test';
import { UssdSimulatorClient } from '../../fixtures/terroir/ussd-simulator-client';

const SERVICE_CODE = '*123#';

function randMsisdn(): string {
  const tail = Math.floor(Math.random() * 1e6).toString().padStart(6, '0');
  return `+22670${tail}`;
}

function randSession(): string {
  return `sess-e2e-${Date.now()}-${Math.floor(Math.random() * 1e6)}`;
}

function randNin(): string {
  return `BF-${Math.floor(Math.random() * 1e10).toString().padStart(10, '0')}`;
}

test.describe('TERROIR P0.F — USSD simulator round-trip', () => {
  let sim: UssdSimulatorClient;
  let reachable = false;

  test.beforeAll(async () => {
    sim = new UssdSimulatorClient();
    reachable = await sim.isReachable();
  });

  test.beforeEach(async ({}, testInfo) => {
    if (!reachable) {
      testInfo.skip(
        true,
        'terroir-ussd-simulator :1080 unreachable — run /cycle-fix first',
      );
    }
    await sim.clearAll();
  });

  test('happy path — Hub2 producer-signup 5 steps + OTP capture', async () => {
    const sessionId = randSession();
    const msisdn = randMsisdn();
    const nin = randNin();

    // Step 1 : ouverture menu (text vide).
    const s1 = await sim.pushHub2({
      session_id: sessionId,
      msisdn,
      service_code: SERVICE_CODE,
      text: '',
    });
    expect(s1.kind).toBe('CON');

    // Step 2 : choix "1" (inscription producteur).
    const s2 = await sim.pushHub2({
      session_id: sessionId,
      msisdn,
      service_code: SERVICE_CODE,
      text: '1',
    });
    expect(s2.kind).toBe('CON');

    // Step 3 : NIN.
    const s3 = await sim.pushHub2({
      session_id: sessionId,
      msisdn,
      service_code: SERVICE_CODE,
      text: `1*${nin}`,
    });
    expect(s3.kind).toBe('CON');

    // Step 4 : nom complet.
    const s4 = await sim.pushHub2({
      session_id: sessionId,
      msisdn,
      service_code: SERVICE_CODE,
      text: `1*${nin}*Aminata Ouedraogo`,
    });
    expect(s4.kind).toBe('CON');

    // Step 4-bis : capturer l'OTP envoyé par SMS.
    const sms = await sim.lastSms(msisdn);
    expect(sms).not.toBeNull();
    expect(sms!.otp_extracted).toMatch(/^\d{8}$/);
    const otp = sms!.otp_extracted!;

    // Step 5 : saisir l'OTP correct → END.
    const s5 = await sim.pushHub2({
      session_id: sessionId,
      msisdn,
      service_code: SERVICE_CODE,
      text: `1*${nin}*Aminata Ouedraogo*${otp}`,
    });
    expect(s5.kind).toBe('END');
    // Accept both accented and ASCII forms — Hub2 mock strips accents to
    // match GSM-7 USSD encoding constraints.
    expect(s5.message).toMatch(/Inscription valid[ée]e/);

    // clearAll() → lastSms doit retourner null.
    await sim.clearAll();
    const after = await sim.lastSms(msisdn);
    expect(after).toBeNull();
  });

  test('error — wrong OTP returns END Code OTP invalide ou expire', async () => {
    const sessionId = randSession();
    const msisdn = randMsisdn();
    const nin = randNin();

    await sim.executeFlow(sessionId, msisdn, SERVICE_CODE, [
      '',
      '1',
      `1*${nin}`,
      `1*${nin}*Test User`,
    ]);

    // Saisi un OTP aléatoire (≠ celui généré).
    const wrongOtp = '99999999';
    const sms = await sim.lastSms(msisdn);
    if (sms?.otp_extracted === wrongOtp) {
      // Ultra-improbable mais on bump pour rester déterministe.
      await sim.clearAll();
      test.skip(true, 'collision OTP improbable — re-run');
    }

    const final = await sim.pushHub2({
      session_id: sessionId,
      msisdn,
      service_code: SERVICE_CODE,
      text: `1*${nin}*Test User*${wrongOtp}`,
    });
    expect(final.kind).toBe('END');
    expect(final.message).toMatch(/OTP invalide|expir/i);
  });

  test('cross-provider — Twilio sendSms indexed in /admin/last-sms', async () => {
    const msisdn = randMsisdn();
    const otp8 = '12345678';

    await sim.sendTwilioSms({
      To: msisdn,
      From: '+15555550100',
      Body: `Votre code de verification: ${otp8} (expire dans 5 min).`,
    });

    const sms = await sim.lastSms(msisdn);
    expect(sms).not.toBeNull();
    expect(sms!.otp_extracted).toBe(otp8);
    expect(sms!.provider).toBe('twilio');
  });
});
