// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Persistance sécurisée du JWT Kratos (et autres secrets agents) via
 * expo-secure-store (Android Keystore + iOS Keychain).
 *
 * Schéma de stockage :
 * - `terroir.jwt` : token Kratos (signed JWS, expiration côté Kratos).
 * - `terroir.refresh_token` : (optionnel) Kratos session_token long-lived.
 * - `terroir.dek_ciphertext` : ciphertext DEK envoyé par Vault Transit
 *   (déchiffré côté serveur uniquement, jamais en clair sur device).
 *
 * Quotas : expo-secure-store limite à 2 KB par valeur sur iOS. JWT Kratos
 * standard ~1 KB, OK. DEK ciphertext ~256 bytes, OK.
 */
import * as SecureStore from 'expo-secure-store';

const KEY_JWT = 'terroir.jwt';
const KEY_REFRESH = 'terroir.refresh_token';
const KEY_DEK = 'terroir.dek_ciphertext';

const SECURE_OPTIONS: SecureStore.SecureStoreOptions = {
  // Sur Android, requireAuthentication=false (sinon prompt biométrie à
  // chaque accès — bloquant pour les sync auto en arrière-plan).
  requireAuthentication: false,
  keychainAccessible: SecureStore.WHEN_UNLOCKED_THIS_DEVICE_ONLY,
};

export async function saveJwt(jwt: string): Promise<void> {
  await SecureStore.setItemAsync(KEY_JWT, jwt, SECURE_OPTIONS);
}

export async function loadJwt(): Promise<string | null> {
  return SecureStore.getItemAsync(KEY_JWT, SECURE_OPTIONS);
}

export async function clearJwt(): Promise<void> {
  await Promise.all([
    SecureStore.deleteItemAsync(KEY_JWT, SECURE_OPTIONS),
    SecureStore.deleteItemAsync(KEY_REFRESH, SECURE_OPTIONS),
  ]);
}

export async function saveRefreshToken(token: string): Promise<void> {
  await SecureStore.setItemAsync(KEY_REFRESH, token, SECURE_OPTIONS);
}

export async function loadRefreshToken(): Promise<string | null> {
  return SecureStore.getItemAsync(KEY_REFRESH, SECURE_OPTIONS);
}

export async function saveDekCiphertext(ciphertext: string): Promise<void> {
  await SecureStore.setItemAsync(KEY_DEK, ciphertext, SECURE_OPTIONS);
}

export async function loadDekCiphertext(): Promise<string | null> {
  return SecureStore.getItemAsync(KEY_DEK, SECURE_OPTIONS);
}

/**
 * Vérifie l'expiration d'un JWT sans bibliothèque de vérification
 * (signature vérifiée côté backend, pas côté device — sinon il faudrait
 * embarquer la JWKS Kratos).
 */
export function isJwtExpired(jwt: string, leewaySeconds = 30): boolean {
  try {
    const parts = jwt.split('.');
    if (parts.length !== 3) {
      return true;
    }
    const payloadB64 = parts[1].replace(/-/g, '+').replace(/_/g, '/');
    const padded = payloadB64 + '='.repeat((4 - (payloadB64.length % 4)) % 4);
    const decoded =
      typeof atob === 'function'
        ? atob(padded)
        : Buffer.from(padded, 'base64').toString('utf-8');
    const payload = JSON.parse(decoded) as { exp?: number };
    if (typeof payload.exp !== 'number') {
      return true;
    }
    const nowSec = Math.floor(Date.now() / 1000);
    return nowSec >= payload.exp - leewaySeconds;
  } catch {
    return true;
  }
}
