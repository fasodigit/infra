// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * POST /api/admin/users/:userId/passkeys/enroll/begin
 * → renvoie le challenge WebAuthn (creation options).
 *
 * Niveau requis : ADMIN (un admin enrolle un user, ou un user pour lui-même).
 */

import { NextResponse, type NextRequest } from 'next/server';

import { adminAuth } from '@/lib/admin-auth';
import { adminProxy } from '@/lib/admin-proxy';
import { extractIdempotencyKey } from '@/lib/admin-otp';
import { WebAuthnRegisterBeginSchema } from '@/lib/schemas/admin';

export async function POST(
  req: NextRequest,
  { params }: { params: Promise<{ userId: string }> },
) {
  const auth = await adminAuth(req, { requiredLevel: 'ADMIN' });
  if (!auth.ok) return auth.response;
  const { userId } = await params;
  const raw = await req.json().catch(() => undefined);
  const parsed = WebAuthnRegisterBeginSchema.safeParse(raw);
  if (!parsed.success) {
    return NextResponse.json({ error: parsed.error.format() }, { status: 400 });
  }
  return adminProxy(auth, {
    method: 'POST',
    path: `/admin/users/${encodeURIComponent(userId)}/passkeys/enroll/begin`,
    body: parsed.data ?? {},
    idempotencyKey: extractIdempotencyKey(req),
  });
}
