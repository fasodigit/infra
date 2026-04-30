// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * POST /api/admin/auth/recovery/initiate
 *
 * **PUBLIC** — pas de session requise (un user qui a perdu son MFA n'a
 * aucune session active).
 *
 * Body : `{ email }` → upstream `auth-ms /admin/auth/recovery/initiate`.
 *
 * Rate-limit : 3 tentatives / heure / IP (in-memory). En production, le
 * seuil fort anti-énumération (§4) est appliqué côté auth-ms qui répond
 * toujours `202 Accepted` (réponse uniforme indépendante de l'existence
 * du compte).
 *
 * Référence : DELTA-REQUIREMENTS-2026-04-30 §4 + §5.
 */

import { NextResponse, type NextRequest } from 'next/server';

import { adminAuth } from '@/lib/admin-auth';
import { adminProxy } from '@/lib/admin-proxy';
import { extractIdempotencyKey } from '@/lib/admin-otp';
import { clientIpFromHeaders, rateLimitCheck } from '@/lib/admin-rate-limit';
import { RecoveryInitiateSchema } from '@/lib/schemas/admin';

const RL_MAX = 3;
const RL_WINDOW_MS = 60 * 60 * 1000;

export async function POST(req: NextRequest) {
  // Rate-limit IP-based (3/h)
  const ip = clientIpFromHeaders(req.headers);
  const rl = rateLimitCheck(`recovery:initiate:${ip}`, RL_MAX, RL_WINDOW_MS);
  if (!rl.allowed) {
    return NextResponse.json(
      { error: 'rate_limit_exceeded' },
      {
        status: 429,
        headers: {
          'Retry-After': String(Math.ceil((rl.resetAt - Date.now()) / 1000)),
        },
      },
    );
  }

  // Pas d'auth check — public
  const auth = await adminAuth(req, { allowPublic: true });
  if (!auth.ok) return auth.response;

  const parsed = RecoveryInitiateSchema.safeParse(await req.json().catch(() => ({})));
  if (!parsed.success) {
    return NextResponse.json({ error: parsed.error.format() }, { status: 400 });
  }
  return adminProxy(auth, {
    method: 'POST',
    target: 'auth-ms',
    path: '/admin/auth/recovery/initiate',
    body: parsed.data,
    idempotencyKey: extractIdempotencyKey(req),
  });
}
