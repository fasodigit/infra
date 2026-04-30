// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * Helpers OTP / idempotence partagés par les routes admin.
 *
 * - generateRequestId() : UUID v4 pour Idempotency-Key et X-Request-Id.
 * - extractIdempotencyKey() : récupère le header client ou en génère un.
 * - timingSafeEqual()      : comparaison constante-en-temps (anti timing
 *                            attack sur les codes OTP / recovery codes).
 */

import type { NextRequest } from 'next/server';

/** Génère un identifiant aléatoire RFC 4122 v4. */
export function generateRequestId(): string {
  // Node 20+/Bun supportent crypto.randomUUID en global.
  if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) {
    return crypto.randomUUID();
  }
  // Fallback (ne devrait jamais frapper en runtime Next 16).
  const bytes = new Uint8Array(16);
  for (let i = 0; i < 16; i++) bytes[i] = Math.floor(Math.random() * 256);
  bytes[6] = (bytes[6]! & 0x0f) | 0x40;
  bytes[8] = (bytes[8]! & 0x3f) | 0x80;
  const hex = Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

/**
 * Extrait `Idempotency-Key` du header. Si absent ou vide, en génère un nouveau.
 * RFC 7231: header insensible à la casse.
 */
export function extractIdempotencyKey(req: NextRequest): string {
  const fromHeader =
    req.headers.get('Idempotency-Key') ??
    req.headers.get('idempotency-key') ??
    req.headers.get('X-Idempotency-Key');
  if (fromHeader && fromHeader.trim().length > 0) return fromHeader.trim();
  return generateRequestId();
}

/**
 * Comparaison timing-safe — protège contre l'inférence de codes OTP/recovery
 * via timing-attack sur la fonction de comparaison. Renvoie `false` si
 * longueurs différentes.
 */
export function timingSafeEqual(a: string, b: string): boolean {
  if (typeof a !== 'string' || typeof b !== 'string') return false;
  if (a.length !== b.length) return false;
  let mismatch = 0;
  for (let i = 0; i < a.length; i++) {
    mismatch |= a.charCodeAt(i) ^ b.charCodeAt(i);
  }
  return mismatch === 0;
}
