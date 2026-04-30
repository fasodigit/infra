// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * POST /api/admin/otp/verify
 * → vérifie un code OTP. Audit : OTP_VERIFIED / OTP_FAILED.
 *
 * Niveau requis : MANAGER.
 */

import { NextResponse, type NextRequest } from 'next/server';

import { adminAuth } from '@/lib/admin-auth';
import { auditLog } from '@/lib/admin-audit';
import { adminProxy } from '@/lib/admin-proxy';
import { extractIdempotencyKey } from '@/lib/admin-otp';
import { OtpVerifySchema } from '@/lib/schemas/admin';

export async function POST(req: NextRequest) {
  const auth = await adminAuth(req, { requiredLevel: 'MANAGER' });
  if (!auth.ok) return auth.response;
  const parsed = OtpVerifySchema.safeParse(await req.json().catch(() => ({})));
  if (!parsed.success) {
    return NextResponse.json({ error: parsed.error.format() }, { status: 400 });
  }
  const idempotencyKey = extractIdempotencyKey(req);
  const res = await adminProxy(auth, {
    method: 'POST',
    path: '/admin/otp/verify',
    body: parsed.data,
    idempotencyKey,
  });
  void auditLog(
    {
      action: res.status >= 200 && res.status < 300 ? 'OTP_VERIFIED' : 'OTP_FAILED',
      actor: { userId: auth.userId, email: auth.email, role: auth.rawRole, ip: auth.ip },
      target: { type: 'otp', id: parsed.data.otpId },
      metadata: { httpStatus: res.status },
      traceId: auth.traceparent,
    },
    { authToken: auth.jwt, idempotencyKey },
  );
  return res;
}
