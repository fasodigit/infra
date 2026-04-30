// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * AdminApiClient — wrapper REST sur les endpoints admin exposés via
 * ARMAGEDDON (`:8080/api/admin/*`) et leur miroir BFF
 * (`:4800/api/admin/*`). Couvre les 25 controllers livrés en Phase 4.b.
 *
 * Authentification :
 *   - Login Kratos `/self-service/login/api` → session_token
 *   - Token transmis via header `X-Session-Token` ou cookie
 *     `ory_kratos_session` selon l'API.
 *
 * Comportement skip-aware :
 *   - `isReachable()` doit toujours être appelé en `beforeAll` ; si false,
 *     `testInfo.skip()` propre.
 *   - Toute méthode renvoie un objet `{status, body, error?}` au lieu de
 *     throw, pour permettre des assertions explicites côté spec.
 */
import { request, type APIRequestContext } from '@playwright/test';

export interface AdminApiResponse<T = unknown> {
  status: number;
  ok: boolean;
  body?: T;
  text?: string;
  error?: string;
}

export interface KratosLoginResult {
  ok: boolean;
  sessionToken?: string;
  identityId?: string;
  error?: string;
}

const DEFAULT_GATEWAY = 'http://localhost:8080';
const DEFAULT_BFF = 'http://localhost:4800';
const DEFAULT_KRATOS = 'http://localhost:4433';

export class AdminApiClient {
  private readonly gatewayURL: string;
  private readonly bffURL: string;
  private readonly kratosURL: string;
  private sessionToken?: string;

  constructor(opts?: { gatewayURL?: string; bffURL?: string; kratosURL?: string }) {
    this.gatewayURL = opts?.gatewayURL ?? process.env.PLAYWRIGHT_BASE_URL ?? DEFAULT_GATEWAY;
    this.bffURL = opts?.bffURL ?? process.env.BFF_URL ?? DEFAULT_BFF;
    this.kratosURL = opts?.kratosURL ?? process.env.KRATOS_PUBLIC_URL ?? DEFAULT_KRATOS;
  }

  setSessionToken(token: string): void {
    this.sessionToken = token;
  }

  getSessionToken(): string | undefined {
    return this.sessionToken;
  }

  private async api(): Promise<APIRequestContext> {
    const headers: Record<string, string> = {
      'content-type': 'application/json',
      accept: 'application/json',
    };
    if (this.sessionToken) {
      headers['x-session-token'] = this.sessionToken;
      headers.authorization = `Bearer ${this.sessionToken}`;
      headers.cookie = `ory_kratos_session=${this.sessionToken}`;
    }
    return request.newContext({ extraHTTPHeaders: headers });
  }

  /** Health check global (gateway + BFF + Kratos). */
  async isReachable(): Promise<{ gateway: boolean; bff: boolean; kratos: boolean }> {
    const out = { gateway: false, bff: false, kratos: false };
    try {
      const a = await request.newContext();
      const r1 = await a.get(`${this.gatewayURL}/`).catch(() => null);
      out.gateway = !!r1 && r1.status() < 500;
      const r2 = await a.get(`${this.bffURL}/`).catch(() => null);
      out.bff = !!r2 && r2.status() < 500;
      const r3 = await a.get(`${this.kratosURL}/health/ready`).catch(() => null);
      out.kratos = !!r3 && r3.ok();
    } catch {
      // ignore
    }
    return out;
  }

  /** Login Kratos via flow API → renvoie un session_token. */
  async login(email: string, password: string): Promise<KratosLoginResult> {
    try {
      const api = await request.newContext();
      const flowRes = await api.get(`${this.kratosURL}/self-service/login/api`);
      if (!flowRes.ok()) return { ok: false, error: `flow init ${flowRes.status()}` };
      const flow = (await flowRes.json()) as { id: string };
      const submitRes = await api.post(
        `${this.kratosURL}/self-service/login?flow=${flow.id}`,
        {
          headers: { 'content-type': 'application/json' },
          data: { method: 'password', identifier: email, password },
        },
      );
      const body = (await submitRes.json()) as {
        session_token?: string;
        session?: { identity?: { id?: string } };
      };
      if (!submitRes.ok() || !body.session_token) {
        return { ok: false, error: `submit ${submitRes.status()}` };
      }
      this.sessionToken = body.session_token;
      return {
        ok: true,
        sessionToken: body.session_token,
        identityId: body.session?.identity?.id,
      };
    } catch (e) {
      return { ok: false, error: e instanceof Error ? e.message : 'unknown' };
    }
  }

  /** Whoami pour vérifier que le token est encore valide. */
  async whoami(): Promise<AdminApiResponse> {
    const api = await this.api();
    try {
      const res = await api.get(`${this.kratosURL}/sessions/whoami`);
      return {
        status: res.status(),
        ok: res.ok(),
        body: res.ok() ? await res.json().catch(() => undefined) : undefined,
        text: res.ok() ? undefined : await res.text().catch(() => undefined),
      };
    } catch (e) {
      return { status: 0, ok: false, error: e instanceof Error ? e.message : 'unknown' };
    }
  }

  // --- Generic gateway/bff requests --------------------------------------

  async get(path: string, opts?: { useBff?: boolean }): Promise<AdminApiResponse> {
    const base = opts?.useBff ? this.bffURL : this.gatewayURL;
    const api = await this.api();
    try {
      const res = await api.get(`${base}${path}`);
      const text = await res.text().catch(() => undefined);
      let body: unknown;
      try {
        body = text ? JSON.parse(text) : undefined;
      } catch {
        body = undefined;
      }
      return { status: res.status(), ok: res.ok(), body, text };
    } catch (e) {
      return { status: 0, ok: false, error: e instanceof Error ? e.message : 'unknown' };
    }
  }

  async post(
    path: string,
    data?: unknown,
    opts?: { useBff?: boolean; idempotencyKey?: string; extraHeaders?: Record<string, string> },
  ): Promise<AdminApiResponse> {
    const base = opts?.useBff ? this.bffURL : this.gatewayURL;
    const headers: Record<string, string> = {};
    if (opts?.idempotencyKey) headers['idempotency-key'] = opts.idempotencyKey;
    if (opts?.extraHeaders) Object.assign(headers, opts.extraHeaders);
    const api = await this.api();
    try {
      const res = await api.post(`${base}${path}`, { data: data ?? {}, headers });
      const text = await res.text().catch(() => undefined);
      let body: unknown;
      try {
        body = text ? JSON.parse(text) : undefined;
      } catch {
        body = undefined;
      }
      return { status: res.status(), ok: res.ok(), body, text };
    } catch (e) {
      return { status: 0, ok: false, error: e instanceof Error ? e.message : 'unknown' };
    }
  }

  async put(
    path: string,
    data?: unknown,
    opts?: { useBff?: boolean; idempotencyKey?: string },
  ): Promise<AdminApiResponse> {
    const base = opts?.useBff ? this.bffURL : this.gatewayURL;
    const headers: Record<string, string> = {};
    if (opts?.idempotencyKey) headers['idempotency-key'] = opts.idempotencyKey;
    const api = await this.api();
    try {
      const res = await api.put(`${base}${path}`, { data: data ?? {}, headers });
      const text = await res.text().catch(() => undefined);
      let body: unknown;
      try {
        body = text ? JSON.parse(text) : undefined;
      } catch {
        body = undefined;
      }
      return { status: res.status(), ok: res.ok(), body, text };
    } catch (e) {
      return { status: 0, ok: false, error: e instanceof Error ? e.message : 'unknown' };
    }
  }

  async delete(path: string, opts?: { useBff?: boolean }): Promise<AdminApiResponse> {
    const base = opts?.useBff ? this.bffURL : this.gatewayURL;
    const api = await this.api();
    try {
      const res = await api.delete(`${base}${path}`);
      const text = await res.text().catch(() => undefined);
      let body: unknown;
      try {
        body = text ? JSON.parse(text) : undefined;
      } catch {
        body = undefined;
      }
      return { status: res.status(), ok: res.ok(), body, text };
    } catch (e) {
      return { status: 0, ok: false, error: e instanceof Error ? e.message : 'unknown' };
    }
  }

  // --- Domain shortcuts (M07/M22, M16/M17, M19/M21, M22, M23, M13, M14, M15) ----

  /** OTP issue (M07). */
  issueOtp(payload: { userId?: string; email?: string; purpose: string }): Promise<AdminApiResponse> {
    return this.post('/api/admin/otp/issue', payload);
  }

  /** OTP verify (M07/M02). */
  verifyOtp(payload: { otpId?: string; code: string; userId?: string }): Promise<AdminApiResponse> {
    return this.post('/api/admin/otp/verify', payload);
  }

  /** List users (M22 audit gate). */
  listUsers(): Promise<AdminApiResponse> {
    return this.get('/api/admin/users');
  }

  /** Grant role (M16/M17). */
  grantRole(
    targetUserId: string,
    payload: { role: string; capabilities?: string[]; justification?: string; otpId?: string; otpCode?: string; force?: boolean; stepUpToken?: string },
    opts?: { idempotencyKey?: string },
  ): Promise<AdminApiResponse> {
    return this.post(`/api/admin/users/${targetUserId}/roles/grant`, payload, opts);
  }

  /** Revoke role (M16/M19). */
  revokeRole(targetUserId: string, payload: { role: string; reason?: string }): Promise<AdminApiResponse> {
    return this.post(`/api/admin/users/${targetUserId}/roles/revoke`, payload);
  }

  /** Capability uniqueness check (M18). */
  checkCapabilityUniqueness(payload: { userId: string; capabilities: string[] }): Promise<AdminApiResponse> {
    return this.post('/api/admin/capabilities/check-uniqueness', payload);
  }

  /** Audit query (M22). */
  queryAudit(params: Record<string, string | number | undefined>): Promise<AdminApiResponse> {
    const qs = Object.entries(params)
      .filter(([, v]) => v !== undefined)
      .map(([k, v]) => `${encodeURIComponent(k)}=${encodeURIComponent(String(v))}`)
      .join('&');
    return this.get(`/api/admin/audit${qs ? `?${qs}` : ''}`);
  }

  /** Settings (M23). */
  getSettings(): Promise<AdminApiResponse> {
    return this.get('/api/admin/settings');
  }

  getSetting(key: string): Promise<AdminApiResponse> {
    return this.get(`/api/admin/settings/${encodeURIComponent(key)}`);
  }

  updateSetting(key: string, payload: { value: unknown; version: number; reason?: string }): Promise<AdminApiResponse> {
    return this.put(`/api/admin/settings/${encodeURIComponent(key)}`, payload);
  }

  getSettingHistory(key: string): Promise<AdminApiResponse> {
    return this.get(`/api/admin/settings/${encodeURIComponent(key)}/history`);
  }

  revertSetting(key: string, payload: { toVersion: number; reason: string }): Promise<AdminApiResponse> {
    return this.post(`/api/admin/settings/${encodeURIComponent(key)}/revert`, payload);
  }

  /** Sessions (M22). */
  listSessions(): Promise<AdminApiResponse> {
    return this.get('/api/admin/sessions');
  }

  forceLogout(jti: string): Promise<AdminApiResponse> {
    return this.delete(`/api/admin/sessions/${encodeURIComponent(jti)}`);
  }

  /** Devices (M12). */
  listDevices(): Promise<AdminApiResponse> {
    return this.get('/api/admin/devices');
  }

  trustDevice(deviceId: string): Promise<AdminApiResponse> {
    return this.post(`/api/admin/devices/${encodeURIComponent(deviceId)}/trust`, {});
  }

  /** Break-glass (M07/M15/M22). */
  activateBreakGlass(payload: { reason: string; ttlMinutes?: number; otpId?: string; otpCode?: string }): Promise<AdminApiResponse> {
    return this.post('/api/admin/break-glass/activate', payload);
  }

  breakGlassStatus(): Promise<AdminApiResponse> {
    return this.get('/api/admin/break-glass/status');
  }

  revokeBreakGlass(): Promise<AdminApiResponse> {
    return this.post('/api/admin/break-glass/revoke', {});
  }

  /** Step-up (M15). */
  beginStepUp(payload: { method?: string; intent?: string }): Promise<AdminApiResponse> {
    return this.post('/api/admin/auth/step-up/begin', payload);
  }

  verifyStepUp(sessionId: string, payload: Record<string, unknown>): Promise<AdminApiResponse> {
    return this.post(`/api/admin/auth/step-up/${encodeURIComponent(sessionId)}/verify`, payload);
  }

  stepUpStatus(sessionId: string): Promise<AdminApiResponse> {
    return this.get(`/api/admin/auth/step-up/${encodeURIComponent(sessionId)}/status`);
  }

  /** Push approval (M13). */
  initiatePushApproval(payload: { intent: string; userId?: string }): Promise<AdminApiResponse> {
    return this.post('/api/admin/auth/push-approval/initiate', payload);
  }

  pushApprovalStatus(requestId: string): Promise<AdminApiResponse> {
    return this.get(`/api/admin/auth/push-approval/${encodeURIComponent(requestId)}/status`);
  }

  respondPushApproval(requestId: string, payload: { selectedNumber: string; granted: boolean }): Promise<AdminApiResponse> {
    return this.post(`/api/admin/auth/push-approval/${encodeURIComponent(requestId)}/respond`, payload);
  }

  /** Account recovery (M20/M21). */
  initiateSelfRecovery(payload: { email: string }): Promise<AdminApiResponse> {
    return this.post('/api/admin/auth/recovery/initiate', payload);
  }

  verifyRecoveryLink(payload: { token: string }): Promise<AdminApiResponse> {
    return this.post('/api/admin/auth/recovery/verify-link', payload);
  }

  verifyRecoveryOtp(payload: { sessionId: string; code: string }): Promise<AdminApiResponse> {
    return this.post('/api/admin/auth/recovery/verify-otp', payload);
  }

  completeRecovery(payload: { sessionId: string; adminToken?: string; email?: string }): Promise<AdminApiResponse> {
    return this.post('/api/admin/auth/recovery/complete', payload);
  }

  initiateAdminRecovery(targetUserId: string, payload: { reason: string; stepUpToken?: string }): Promise<AdminApiResponse> {
    return this.post(`/api/admin/users/${encodeURIComponent(targetUserId)}/recovery/initiate`, payload);
  }

  /** Login risk (M14). */
  reportLoginRisk(payload: { email: string; ip?: string; userAgent?: string }): Promise<AdminApiResponse> {
    return this.post('/api/admin/auth/login/risk', payload);
  }

  /** Recovery codes (M11). */
  generateRecoveryCodes(targetUserId: string): Promise<AdminApiResponse> {
    return this.post(`/api/admin/users/${encodeURIComponent(targetUserId)}/recovery-codes/generate`, {});
  }

  useRecoveryCode(payload: { code: string; email?: string }): Promise<AdminApiResponse> {
    return this.post('/api/admin/recovery-codes/use', payload);
  }

  loginWithRecoveryCode(payload: { code: string; email: string }): Promise<AdminApiResponse> {
    return this.post('/api/admin/auth/login/recovery-code', payload);
  }

  /** TOTP (M10/M04). */
  beginTotpEnroll(): Promise<AdminApiResponse> {
    return this.post('/api/admin/me/totp/enroll/begin', {});
  }

  finishTotpEnroll(payload: { code: string; secret?: string }): Promise<AdminApiResponse> {
    return this.post('/api/admin/me/totp/enroll/finish', payload);
  }

  deleteOwnTotp(): Promise<AdminApiResponse> {
    return this.delete('/api/admin/me/totp');
  }

  /** PassKey (M09). */
  beginPasskeyEnroll(): Promise<AdminApiResponse> {
    return this.post('/api/admin/me/passkeys/enroll/begin', {});
  }

  finishPasskeyEnroll(payload: Record<string, unknown>): Promise<AdminApiResponse> {
    return this.post('/api/admin/me/passkeys/enroll/finish', payload);
  }

  /** Self-mgmt (M01). */
  changeOwnPassword(payload: { current: string; next: string }): Promise<AdminApiResponse> {
    return this.post('/api/admin/me/password', payload);
  }

  /** Capabilities catalog (M17). */
  listCapabilities(): Promise<AdminApiResponse> {
    return this.get('/api/admin/capabilities');
  }

  /** Onboarding (M06). */
  beginOnboard(payload: { token: string }): Promise<AdminApiResponse> {
    return this.post('/api/admin/auth/onboard/begin', payload);
  }

  verifyOnboardLink(payload: { token: string }): Promise<AdminApiResponse> {
    return this.post('/api/admin/auth/onboard/verify-link', payload);
  }

  verifyOnboardOtp(payload: { sessionId: string; code: string }): Promise<AdminApiResponse> {
    return this.post('/api/admin/auth/onboard/verify-otp', payload);
  }
}
