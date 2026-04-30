// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * POST /api/admin/auth/onboard/verify-link
 *
 * Phase 4.b.4 §6 — Magic-link channel-binding (étape 2/3).
 *
 * **PUBLIC** — pas de session active : l'utilisateur clique sur le lien
 * magique de l'email d'invitation. Le BFF transmet le JWT à auth-ms qui
 * vérifie signature + JTI single-use puis génère un OTP 8 chiffres affiché
 * sur la page (channel-binding).
 *
 * Rate-limit : 10 tentatives / heure / IP.
 */

import { NextResponse, type NextRequest } from 'next/server';

import { adminAuth } from '@/lib/admin-auth';
import { adminProxy } from '@/lib/admin-proxy';
import { clientIpFromHeaders, rateLimitCheck } from '@/lib/admin-rate-limit';
import { OnboardVerifyLinkSchema } from '@/lib/schemas/admin';

const RL_MAX = 10;
const RL_WINDOW_MS = 60 * 60 * 1000;

export async function POST(req: NextRequest) {
  const ip = clientIpFromHeaders(req.headers);
  const rl = rateLimitCheck(`onboard:verify-link:${ip}`, RL_MAX, RL_WINDOW_MS);
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

  const parsed = OnboardVerifyLinkSchema.safeParse(await req.json().catch(() => ({})));
  if (!parsed.success) {
    return NextResponse.json({ error: parsed.error.format() }, { status: 400 });
  }
  return adminProxy(auth, {
    method: 'POST',
    target: 'auth-ms',
    path: '/admin/auth/onboard/verify-link',
    body: parsed.data,
  });
}
