// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Flow Kratos via terroir-mobile-bff (proxy ARMAGEDDON :8080).
 *
 * Le BFF abstrait Kratos pour mobile :
 *   POST /auth/login/init   → retourne flowId + identifié required_methods
 *   POST /auth/login/submit → email/password + (optionnel) totp_code
 *                             retourne JWT 14j sliding (cf. Q2)
 *   POST /auth/login/mfa    → 2nd facteur (TOTP/passkey) si required
 *
 * Sliding 14j : à chaque sync (`/m/sync/batch`) le BFF rafraîchit la session
 * Kratos ; pas de refresh token explicite côté mobile.
 *
 * P3+ : enrôlement passkey via virtualauthenticator. P0/P1 : password + TOTP.
 */
import { apiClient, ApiClientError } from '../api/client';
import type { AuthSession } from '../api/types';
import { clearJwt, saveJwt, saveRefreshToken } from './jwt-storage';

export type MfaMethod = 'totp' | 'passkey';

export interface InitLoginRequest {
  email: string;
}

export interface InitLoginResponse {
  flowId: string;
  /** Méthodes de second facteur acceptées par Kratos (vide = password seulement). */
  requiredMethods: MfaMethod[];
}

export interface SubmitLoginRequest {
  flowId: string;
  email: string;
  password: string;
  totpCode?: string;
}

export interface MfaChallengeRequest {
  flowId: string;
  method: MfaMethod;
  code: string;
}

export interface LoginSuccessResponse {
  session: AuthSession;
}

/**
 * Démarre un flow login Kratos. Retourne flowId + listes des MFA exigés.
 */
export async function initLoginFlow(req: InitLoginRequest): Promise<InitLoginResponse> {
  return apiClient.post<InitLoginResponse>('/auth/login/init', req);
}

/**
 * Soumet email/password (+ TOTP si présent). Si Kratos exige un MFA séparé,
 * rejette avec status 401 + code `mfa_required` — appeler `submitMfaChallenge`.
 */
export async function submitLogin(req: SubmitLoginRequest): Promise<AuthSession> {
  const response = await apiClient.post<LoginSuccessResponse>('/auth/login/submit', req);
  await saveJwt(response.session.jwt);
  if (response.session.refresh_token) {
    await saveRefreshToken(response.session.refresh_token);
  }
  return response.session;
}

/**
 * Soumet 2nd facteur (TOTP ou WebAuthn assertion). Retourne session JWT.
 */
export async function submitMfaChallenge(req: MfaChallengeRequest): Promise<AuthSession> {
  const response = await apiClient.post<LoginSuccessResponse>('/auth/login/mfa', req);
  await saveJwt(response.session.jwt);
  return response.session;
}

/**
 * Login complet en une étape : tente submit, et retourne `mfaRequired=true` si
 * le BFF répond 401/`mfa_required` — l'écran demandera alors le code TOTP.
 */
export type LoginAttempt =
  | { ok: true; session: AuthSession }
  | { ok: false; mfaRequired: true; flowId: string; methods: MfaMethod[] }
  | { ok: false; mfaRequired: false; reason: string };

export async function attemptLogin(email: string, password: string): Promise<LoginAttempt> {
  let init: InitLoginResponse;
  try {
    init = await initLoginFlow({ email });
  } catch (err) {
    if (err instanceof ApiClientError) {
      return { ok: false, mfaRequired: false, reason: err.message };
    }
    throw err;
  }

  try {
    const session = await submitLogin({
      flowId: init.flowId,
      email,
      password,
    });
    return { ok: true, session };
  } catch (err) {
    if (err instanceof ApiClientError) {
      if (err.status === 401 && err.apiError?.code === 'mfa_required') {
        return {
          ok: false,
          mfaRequired: true,
          flowId: init.flowId,
          methods: init.requiredMethods,
        };
      }
      return { ok: false, mfaRequired: false, reason: err.message };
    }
    throw err;
  }
}

export async function logout(): Promise<void> {
  try {
    await apiClient.post('/auth/logout');
  } catch {
    // best-effort
  } finally {
    await clearJwt();
  }
}
