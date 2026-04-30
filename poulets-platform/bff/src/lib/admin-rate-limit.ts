// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * Rate-limiter IP simpliste in-memory pour les endpoints **publics** du BFF
 * (recovery flows). Sliding window via `Map<ip, timestamps[]>`.
 *
 * NOTE — souveraineté : à terme, doit être migré sur **KAYA** (sovereign
 * sliding-window counter atomic via SCRIPT). En attendant, l'in-memory est
 * acceptable car le BFF tourne en single-replica derrière ARMAGEDDON ; en
 * multi-replica le rate-limit fort est appliqué côté auth-ms (source de
 * vérité pour les seuils anti-énumération §4).
 *
 * TODO(KAYA) : remplacer par un client `@faso/kaya-client` quand le SDK
 * Node sera publié (cf. INFRA/kaya/clients/ts).
 */

interface SlidingWindowEntry {
  hits: number[];
}

const STORE_GLOBAL = globalThis as typeof globalThis & {
  __faso_rl_store__?: Map<string, SlidingWindowEntry>;
};
const store: Map<string, SlidingWindowEntry> =
  STORE_GLOBAL.__faso_rl_store__ ?? (STORE_GLOBAL.__faso_rl_store__ = new Map());

export interface RateLimitResult {
  allowed: boolean;
  remaining: number;
  resetAt: number;
}

/**
 * Vérifie qu'une clé (typiquement `<scope>:<ip>`) n'a pas dépassé
 * `maxAttempts` dans la fenêtre `windowMs`. Sinon refuse.
 */
export function rateLimitCheck(
  key: string,
  maxAttempts: number,
  windowMs: number,
): RateLimitResult {
  const now = Date.now();
  const entry = store.get(key) ?? { hits: [] };
  // Purge expired
  entry.hits = entry.hits.filter((ts) => now - ts < windowMs);
  if (entry.hits.length >= maxAttempts) {
    const oldest = entry.hits[0] ?? now;
    return { allowed: false, remaining: 0, resetAt: oldest + windowMs };
  }
  entry.hits.push(now);
  store.set(key, entry);
  return {
    allowed: true,
    remaining: Math.max(0, maxAttempts - entry.hits.length),
    resetAt: now + windowMs,
  };
}

/** Extrait l'IP cliente d'un Request (X-Forwarded-For en priorité). */
export function clientIpFromHeaders(headers: Headers): string {
  const xff = headers.get('x-forwarded-for');
  if (xff) {
    const first = xff.split(',')[0];
    if (first) return first.trim();
  }
  const realIp = headers.get('x-real-ip');
  if (realIp) return realIp.trim();
  return 'unknown';
}
