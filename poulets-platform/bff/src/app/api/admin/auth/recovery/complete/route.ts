// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * POST /api/admin/auth/recovery/complete
 *
 * **PUBLIC** — l'utilisateur soumet le token reçu par email + (optionnel)
 * l'identifiant du flow Kratos pour finaliser la récupération.
 *
 * Body : `{ tokenOrCode, kratosFlowId? }` → upstream
 * `auth-ms /admin/auth/recovery/complete`.
 *
 * Référence : DELTA-REQUIREMENTS-2026-04-30 §4 + §5.
 */

import { NextResponse, type NextRequest } from 'next/server';

import { adminAuth } from '@/lib/admin-auth';
import { adminProxy } from '@/lib/admin-proxy';
import { extractIdempotencyKey } from '@/lib/admin-otp';
import { clientIpFromHeaders, rateLimitCheck } from '@/lib/admin-rate-limit';
import { RecoveryCompleteSchema } from '@/lib/schemas/admin';

const RL_MAX = 10;
const RL_WINDOW_MS = 60 * 60 * 1000;

export async function POST(req: NextRequest) {
  const ip = clientIpFromHeaders(req.headers);
  const rl = rateLimitCheck(`recovery:complete:${ip}`, RL_MAX, RL_WINDOW_MS);
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

  const auth = await adminAuth(req, { allowPublic: true });
  if (!auth.ok) return auth.response;

  const parsed = RecoveryCompleteSchema.safeParse(await req.json().catch(() => ({})));
  if (!parsed.success) {
    return NextResponse.json({ error: parsed.error.format() }, { status: 400 });
  }
  return adminProxy(auth, {
    method: 'POST',
    target: 'auth-ms',
    path: '/admin/auth/recovery/complete',
    body: parsed.data,
    idempotencyKey: extractIdempotencyKey(req),
  });
}
