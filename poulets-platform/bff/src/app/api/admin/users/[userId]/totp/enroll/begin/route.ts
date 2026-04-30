// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * POST /api/admin/users/:userId/totp/enroll/begin
 * → renvoie le secret TOTP (base32) + URL otpauth:// pour QR.
 *
 * Niveau requis : ADMIN.
 */

import { NextResponse, type NextRequest } from 'next/server';

import { adminAuth } from '@/lib/admin-auth';
import { adminProxy } from '@/lib/admin-proxy';
import { extractIdempotencyKey } from '@/lib/admin-otp';
import { TotpEnrollBeginSchema } from '@/lib/schemas/admin';

export async function POST(
  req: NextRequest,
  { params }: { params: Promise<{ userId: string }> },
) {
  const auth = await adminAuth(req, { requiredLevel: 'ADMIN' });
  if (!auth.ok) return auth.response;
  const { userId } = await params;
  const raw = await req.json().catch(() => undefined);
  const parsed = TotpEnrollBeginSchema.safeParse(raw);
  if (!parsed.success) {
    return NextResponse.json({ error: parsed.error.format() }, { status: 400 });
  }
  return adminProxy(auth, {
    method: 'POST',
    path: `/admin/users/${encodeURIComponent(userId)}/totp/enroll/begin`,
    body: parsed.data ?? {},
    idempotencyKey: extractIdempotencyKey(req),
  });
}
