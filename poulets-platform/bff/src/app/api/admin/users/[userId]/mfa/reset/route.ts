// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * POST /api/admin/users/:userId/mfa/reset — réinitialise MFA (TOTP/passkey/recovery).
 *
 * Niveau requis : ADMIN. Body MfaResetRequestSchema (motif, methods, otpCode).
 */

import { NextResponse, type NextRequest } from 'next/server';

import { adminAuth } from '@/lib/admin-auth';
import { auditLog } from '@/lib/admin-audit';
import { adminProxy } from '@/lib/admin-proxy';
import { extractIdempotencyKey } from '@/lib/admin-otp';
import { MfaResetRequestSchema } from '@/lib/schemas/admin';

export async function POST(
  req: NextRequest,
  { params }: { params: Promise<{ userId: string }> },
) {
  const auth = await adminAuth(req, { requiredLevel: 'ADMIN' });
  if (!auth.ok) return auth.response;
  const { userId } = await params;
  const parsed = MfaResetRequestSchema.safeParse(await req.json().catch(() => ({})));
  if (!parsed.success) {
    return NextResponse.json({ error: parsed.error.format() }, { status: 400 });
  }
  const idempotencyKey = extractIdempotencyKey(req);
  const res = await adminProxy(auth, {
    method: 'POST',
    path: `/admin/users/${encodeURIComponent(userId)}/mfa/reset`,
    body: parsed.data,
    idempotencyKey,
  });
  if (res.status >= 200 && res.status < 300) {
    void auditLog(
      {
        action: 'MFA_REMOVED',
        actor: { userId: auth.userId, email: auth.email, role: auth.rawRole, ip: auth.ip },
        target: { type: 'user', id: userId },
        metadata: { methods: parsed.data.methods, motif: parsed.data.motif },
        traceId: auth.traceparent,
        critical: true,
      },
      { authToken: auth.jwt, idempotencyKey },
    );
  }
  return res;
}
