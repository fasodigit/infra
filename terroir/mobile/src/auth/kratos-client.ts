// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Client Kratos via terroir-mobile-bff (:8833).
 *
 * Le BFF abstrait Kratos pour mobile :
 * - POST /auth/login   (email + password → JWT)
 * - POST /auth/logout  (révocation session)
 * - POST /auth/whoami  (introspection)
 * - POST /auth/refresh (rotation token)
 *
 * P0 : signatures + placeholders.
 * P1 : MFA (TOTP), recover password, magic link, kratos hooks.
 */
import { apiClient } from '../api/client';
import type { AuthSession } from '../api/types';
import { clearJwt, saveJwt, saveRefreshToken } from './jwt-storage';

export interface LoginCredentials {
  email: string;
  password: string;
  totp_code?: string;
}

export interface LoginResponse {
  session: AuthSession;
}

export async function login(credentials: LoginCredentials): Promise<AuthSession> {
  const response = await apiClient.post<LoginResponse>('/auth/login', credentials);
  await saveJwt(response.session.jwt);
  if (response.session.refresh_token) {
    await saveRefreshToken(response.session.refresh_token);
  }
  return response.session;
}

export async function logout(): Promise<void> {
  try {
    await apiClient.post('/auth/logout');
  } catch {
    // Best-effort : si offline ou JWT déjà invalide, on nettoie quand même.
  } finally {
    await clearJwt();
  }
}

export async function whoami(): Promise<AuthSession | null> {
  try {
    const response = await apiClient.get<LoginResponse>('/auth/whoami');
    return response.session;
  } catch {
    return null;
  }
}
