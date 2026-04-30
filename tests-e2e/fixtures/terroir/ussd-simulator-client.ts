// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * UssdSimulatorClient — wrapper du service `terroir-ussd-simulator :1080`.
 *
 * Couvre les endpoints P0.F documentés dans
 * `INFRA/terroir/docs/ULTRAPLAN-TERROIR-2026-04-30.md` §4 P0.6 :
 *   - POST /hub2/ussd/push        (provider Hub2 mock)
 *   - POST /africastalking/ussd   (provider AT mock)
 *   - POST /twilio/sms/send       (provider Twilio mock)
 *   - GET  /admin/last-sms        (capture OTP — regex 8 digits)
 *   - DELETE /admin/clear         (reset complet, idempotent)
 *   - GET  /admin/sessions/:id    (état d'une session USSD)
 *
 * Pas de mocks au niveau Playwright : on parle au binaire Rust loopback
 * démarré par `cycle-fix`. Les "providers mock" sont les implémentations
 * fournies par le simulator Rust lui-même (P0 prep avant choix Hub2/AT).
 */
import { request, type APIRequestContext } from '@playwright/test';

export interface Hub2PushRequest {
  session_id: string;
  msisdn: string;
  service_code: string;
  text: string;
}

export interface Hub2PushResponse {
  /** Toujours `CON ...` ou `END ...` (USSD canonique). */
  message: string;
  /** Type du retour : `CON` continue, `END` termine la session. */
  kind: 'CON' | 'END';
  session_id: string;
}

export interface AfricasTalkingUssdRequest {
  sessionId: string;
  serviceCode: string;
  phoneNumber: string;
  text: string;
}

export interface TwilioSmsForm {
  To: string;
  From: string;
  Body: string;
}

export interface LastSmsResponse {
  msisdn: string;
  body: string;
  /** Premier groupe `\b(\d{8})\b` extrait du body, ou `null` si rien. */
  otp_extracted: string | null;
  provider: 'hub2' | 'africastalking' | 'twilio' | 'unknown';
  received_at: string;
}

export interface UssdSession {
  session_id: string;
  msisdn: string;
  current_step: string;
  step_index: number;
  data: Record<string, string>;
  started_at: string;
  last_input_at: string;
}

const OTP_REGEX_8 = /\b(\d{8})\b/;

export class UssdSimulatorClient {
  private readonly baseURL: string;

  constructor(baseURL?: string) {
    this.baseURL =
      baseURL ?? process.env.TERROIR_USSD_SIMULATOR_URL ?? 'http://localhost:1080';
  }

  private async api(): Promise<APIRequestContext> {
    return request.newContext();
  }

  /** Push une étape Hub2 et parse la réponse Hub2 mock. */
  async pushHub2(req: Hub2PushRequest): Promise<Hub2PushResponse> {
    const api = await this.api();
    const res = await api.post(`${this.baseURL}/hub2/ussd/push`, {
      data: req,
      headers: { 'content-type': 'application/json' },
    });
    if (!res.ok()) {
      throw new Error(
        `Hub2 push HTTP ${res.status()} : ${await res.text()}`,
      );
    }
    // Hub2 mock returns:
    //   { sessionId, status: "OK"|"FAILED", message, end: bool, level }
    // The CON/END prefix model used by Africa's Talking is mapped onto
    // the boolean `end` flag here.
    const json = (await res.json()) as {
      sessionId?: string;
      message?: string;
      end?: boolean;
      level?: number;
    };
    const raw = json.message ?? '';
    const kind: 'CON' | 'END' = json.end === true ? 'END' : 'CON';
    return {
      message: raw,
      kind,
      session_id: json.sessionId ?? req.session_id,
    };
  }

  /** Push une étape AfricasTalking (form-encoded canonique). */
  async pushAfricasTalking(req: AfricasTalkingUssdRequest): Promise<string> {
    const api = await this.api();
    const form = new URLSearchParams({
      sessionId: req.sessionId,
      serviceCode: req.serviceCode,
      phoneNumber: req.phoneNumber,
      text: req.text,
    });
    const res = await api.post(`${this.baseURL}/africastalking/ussd`, {
      headers: { 'content-type': 'application/x-www-form-urlencoded' },
      data: form.toString(),
    });
    if (!res.ok()) {
      throw new Error(`AT push HTTP ${res.status()}`);
    }
    return res.text();
  }

  /** Envoi un SMS via le provider Twilio mock (form-encoded). */
  async sendTwilioSms(req: TwilioSmsForm): Promise<{ sid: string }> {
    const api = await this.api();
    const form = new URLSearchParams({
      To: req.To,
      From: req.From,
      Body: req.Body,
    });
    const res = await api.post(`${this.baseURL}/twilio/sms/send`, {
      headers: { 'content-type': 'application/x-www-form-urlencoded' },
      data: form.toString(),
    });
    if (!res.ok()) {
      throw new Error(`Twilio SMS HTTP ${res.status()}`);
    }
    const body = (await res.json()) as { sid?: string };
    return { sid: body.sid ?? '' };
  }

  /**
   * Récupère le dernier SMS pour un MSISDN donné. Retourne `null` si
   * le simulator n'a rien capturé. Le champ `otp_extracted` est extrait
   * via la regex `\b(\d{8})\b` côté simulator (P0.F spec) — on revérifie
   * côté client pour robustesse.
   */
  async lastSms(msisdn: string): Promise<LastSmsResponse | null> {
    const api = await this.api();
    const res = await api.get(
      `${this.baseURL}/admin/last-sms?msisdn=${encodeURIComponent(msisdn)}`,
    );
    if (res.status() === 404) return null;
    if (!res.ok()) {
      throw new Error(`last-sms HTTP ${res.status()}`);
    }
    const json = (await res.json()) as Partial<LastSmsResponse>;
    if (!json || !json.body) return null;
    const otpFromBody = json.body.match(OTP_REGEX_8);
    return {
      msisdn: json.msisdn ?? msisdn,
      body: json.body,
      otp_extracted: json.otp_extracted ?? (otpFromBody ? otpFromBody[1]! : null),
      provider: (json.provider as LastSmsResponse['provider']) ?? 'unknown',
      received_at: json.received_at ?? new Date().toISOString(),
    };
  }

  async getSession(sessionId: string): Promise<UssdSession | null> {
    const api = await this.api();
    const res = await api.get(
      `${this.baseURL}/admin/sessions/${encodeURIComponent(sessionId)}`,
    );
    if (res.status() === 404) return null;
    if (!res.ok()) {
      throw new Error(`getSession HTTP ${res.status()}`);
    }
    return (await res.json()) as UssdSession;
  }

  /** Reset complet : vide les SMS capturés et toutes les sessions. */
  async clearAll(): Promise<void> {
    const api = await this.api();
    // Server exposes POST /admin/clear (not DELETE) — see admin.rs route.
    const res = await api.post(`${this.baseURL}/admin/clear`);
    if (!res.ok()) {
      throw new Error(`clearAll HTTP ${res.status()}`);
    }
  }

  /**
   * Helper : exécute un flow complet en envoyant N steps Hub2 dans la
   * même session_id. Renvoie la dernière réponse (souvent un `END ...`).
   */
  async executeFlow(
    sessionId: string,
    msisdn: string,
    serviceCode: string,
    steps: string[],
  ): Promise<Hub2PushResponse> {
    let last: Hub2PushResponse | null = null;
    for (const text of steps) {
      last = await this.pushHub2({
        session_id: sessionId,
        msisdn,
        service_code: serviceCode,
        text,
      });
    }
    if (!last) {
      throw new Error('executeFlow: aucun step fourni');
    }
    return last;
  }

  async isReachable(): Promise<boolean> {
    try {
      const api = await this.api();
      // terroir-ussd-simulator exposes /health/ready (Axum probe) ;
      // an /admin/health alias is not provided in P0.F.
      const res = await api.get(`${this.baseURL}/health/ready`);
      return res.ok();
    } catch {
      return false;
    }
  }
}
