// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * POST /api/admin/auth/onboard/begin
 *
 * Phase 4.b.4 — Magic-link channel-binding au signup admin.
 *
 * **PUBLIC** — appelé par le BFF UI lorsque le SUPER-ADMIN clique sur
 * "Inviter un admin". L'auth super-admin elle-même est validée par la route
 * UI parente (route `/admin/users/invite`) qui pré-alloue l'`invitationId`
 * et appelle ensuite ce endpoint pour émettre le magic-link signé.
 *
 * Les schémas Zod assurent que seuls les payloads attendus passent au
 * upstream (`adminProxy` ne fait pas de validation de body).
 */

import { NextResponse, type NextRequest } from 'next/server';

import { adminAuth } from '@/lib/admin-auth';
import { adminProxy } from '@/lib/admin-proxy';
import { extractIdempotencyKey } from '@/lib/admin-otp';
import { clientIpFromHeaders, rateLimitCheck } from '@/lib/admin-rate-limit';
import { OnboardBeginSchema } from '@/lib/schemas/admin';

const RL_MAX = 10;
const RL_WINDOW_MS = 60 * 60 * 1000;

export async function POST(req: NextRequest) {
  const ip = clientIpFromHeaders(req.headers);
  const rl = rateLimitCheck(`onboard:begin:${ip}`, RL_MAX, RL_WINDOW_MS);
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

  // The endpoint accepts unauthenticated traffic so the SUPER-ADMIN UI
  // (which orchestrates the invitation flow) can proxy without round-tripping
  // through the JWT-only path. Final auth-ms still trusts the call because
  // mTLS-SVID is enforced at ARMAGEDDON.
  const auth = await adminAuth(req, { allowPublic: true });
  if (!auth.ok) return auth.response;

  const parsed = OnboardBeginSchema.safeParse(await req.json().catch(() => ({})));
  if (!parsed.success) {
    return NextResponse.json({ error: parsed.error.format() }, { status: 400 });
  }
  return adminProxy(auth, {
    method: 'POST',
    target: 'auth-ms',
    path: '/admin/auth/onboard/begin',
    body: parsed.data,
    idempotencyKey: extractIdempotencyKey(req),
  });
}
