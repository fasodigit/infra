// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * POST /api/admin/auth/step-up/{sessionId}/verify
 *
 * Vérifie la preuve (passkey assertion / push requestId / TOTP / OTP) et,
 * si OK, renvoie un `{ stepUpToken }` court (TTL 5 min) que le frontend
 * réutilise sur la requête originale (header `Authorization: Bearer ...`).
 *
 * Référence : SECURITY-HARDENING-PLAN-2026-04-30 §4 Tier 4 + §6 Phase 4.b.7.
 */

import { NextResponse, type NextRequest } from 'next/server';

import { adminAuth } from '@/lib/admin-auth';
import { adminProxy } from '@/lib/admin-proxy';
import { extractIdempotencyKey } from '@/lib/admin-otp';
import { StepUpVerifySchema } from '@/lib/schemas/admin';

export async function POST(
  req: NextRequest,
  ctx: { params: Promise<{ sessionId: string }> },
) {
  const auth = await adminAuth(req, { requiredLevel: 'MANAGER' });
  if (!auth.ok) return auth.response;

  const { sessionId } = await ctx.params;
  if (!sessionId || sessionId.length < 8) {
    return NextResponse.json({ error: 'invalid_session_id' }, { status: 400 });
  }

  const parsed = StepUpVerifySchema.safeParse(await req.json().catch(() => ({})));
  if (!parsed.success) {
    return NextResponse.json({ error: parsed.error.format() }, { status: 400 });
  }

  return adminProxy(auth, {
    method: 'POST',
    target: 'auth-ms',
    path: `/admin/auth/step-up/${encodeURIComponent(sessionId)}/verify`,
    body: parsed.data,
    idempotencyKey: extractIdempotencyKey(req),
  });
}
