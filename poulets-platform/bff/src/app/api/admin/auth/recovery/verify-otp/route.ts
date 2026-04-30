// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * POST /api/admin/auth/recovery/verify-otp
 *
 * Phase 4.b.4 §3 — finalise le flow self-recovery magic-link en consommant
 * la session KAYA créée par {@code verify-link}. auth-ms marque la requête
 * USED, applique `must_reenroll_mfa = true` et renvoie un descriptor AAL1
 * (le BFF complète ensuite avec un Kratos settings flow).
 *
 * **PUBLIC** — pas de session active.
 * Rate-limit : 10 tentatives / heure / IP.
 */

import { NextResponse, type NextRequest } from 'next/server';

import { adminAuth } from '@/lib/admin-auth';
import { adminProxy } from '@/lib/admin-proxy';
import { clientIpFromHeaders, rateLimitCheck } from '@/lib/admin-rate-limit';
import { RecoveryVerifyOtpSchema } from '@/lib/schemas/admin';

const RL_MAX = 10;
const RL_WINDOW_MS = 60 * 60 * 1000;

export async function POST(req: NextRequest) {
  const ip = clientIpFromHeaders(req.headers);
  const rl = rateLimitCheck(`recovery:verify-otp:${ip}`, RL_MAX, RL_WINDOW_MS);
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

  const parsed = RecoveryVerifyOtpSchema.safeParse(await req.json().catch(() => ({})));
  if (!parsed.success) {
    return NextResponse.json({ error: parsed.error.format() }, { status: 400 });
  }
  return adminProxy(auth, {
    method: 'POST',
    target: 'auth-ms',
    path: '/admin/auth/recovery/verify-otp',
    body: parsed.data,
  });
}
