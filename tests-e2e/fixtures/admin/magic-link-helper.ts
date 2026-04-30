// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * MagicLinkHelper — extrait le token JWT (M06) d'un email Mailpit puis
 * le parse pour exposer les params utiles aux specs Phase 4.d :
 *   - URL complète depuis le HTML de l'email
 *   - paramètre `token` (JWT signé HMAC, single-use, TTL 30min)
 *   - jti (anti-replay KAYA `auth:onboard:jti:*`)
 *
 * Skip-aware : si Mailpit injoignable, renvoie {unavailable:true} au lieu
 * de throw — les specs doivent appeler `isReachable()` au préalable.
 */
import { MailpitClient } from '../mailpit';

export interface ExtractedMagicLink {
  unavailable?: boolean;
  reason?: string;
  url?: string;
  token?: string;
  jti?: string;
  email?: string;
  exp?: number;
}

export class MagicLinkHelper {
  private readonly mailpit: MailpitClient;

  constructor(mailpit?: MailpitClient) {
    this.mailpit = mailpit ?? new MailpitClient();
  }

  /** Décode un JWT sans vérifier la signature (côté E2E uniquement). */
  static decodeJwtPayload(token: string): Record<string, unknown> | null {
    try {
      const parts = token.split('.');
      if (parts.length < 2 || !parts[1]) return null;
      const padded = parts[1].replace(/-/g, '+').replace(/_/g, '/');
      const pad = padded.length % 4;
      const fixed = pad ? padded + '='.repeat(4 - pad) : padded;
      const json = Buffer.from(fixed, 'base64').toString('utf-8');
      return JSON.parse(json) as Record<string, unknown>;
    } catch {
      return null;
    }
  }

  /** Extrait un magic-link envoyé à `email`. */
  async waitForMagicLink(
    email: string,
    opts: { timeoutMs?: number; urlRegex?: RegExp } = {},
  ): Promise<ExtractedMagicLink> {
    const reachable = await this.mailpit.isReachable();
    if (!reachable) return { unavailable: true, reason: 'mailpit-down' };
    const urlRegex =
      opts.urlRegex ?? /(https?:\/\/[^\s"'<>]+(?:\/auth\/(?:admin-onboard|recovery)|\/onboard|\/recovery)[^\s"'<>]*)/;
    try {
      const url = await this.mailpit.waitForLink(email, {
        urlRegex,
        timeoutMs: opts.timeoutMs ?? 15_000,
        deleteAfter: false,
      });
      const tokenMatch = url.match(/[?&]token=([^&]+)/);
      const token = tokenMatch?.[1];
      if (!token) return { url, reason: 'token-not-found' };
      const payload = MagicLinkHelper.decodeJwtPayload(token);
      return {
        url,
        token,
        jti: typeof payload?.jti === 'string' ? (payload.jti as string) : undefined,
        email: typeof payload?.email === 'string' ? (payload.email as string) : undefined,
        exp: typeof payload?.exp === 'number' ? (payload.exp as number) : undefined,
      };
    } catch (e) {
      return {
        unavailable: true,
        reason: `mailpit-error: ${e instanceof Error ? e.message : 'unknown'}`,
      };
    }
  }

  /** Tampère la signature du JWT (dernière section du `header.payload.sig`). */
  static tamperToken(token: string): string {
    const parts = token.split('.');
    if (parts.length < 3) return token + 'X';
    parts[2] = (parts[2] ?? '').slice(0, -1) + (parts[2]?.endsWith('A') ? 'B' : 'A');
    return parts.join('.');
  }
}
