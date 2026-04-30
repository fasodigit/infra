// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * POST /api/admin/users/:userId/passkeys/:passkeyId/rename
 * → renomme un passkey enrollé.
 *
 * Niveau requis : ADMIN.
 */

import { NextResponse, type NextRequest } from 'next/server';

import { adminAuth } from '@/lib/admin-auth';
import { adminProxy } from '@/lib/admin-proxy';
import { extractIdempotencyKey } from '@/lib/admin-otp';
import { PasskeyRenameSchema } from '@/lib/schemas/admin';

export async function POST(
  req: NextRequest,
  { params }: { params: Promise<{ userId: string; passkeyId: string }> },
) {
  const auth = await adminAuth(req, { requiredLevel: 'ADMIN' });
  if (!auth.ok) return auth.response;
  const { userId, passkeyId } = await params;
  const parsed = PasskeyRenameSchema.safeParse(await req.json().catch(() => ({})));
  if (!parsed.success) {
    return NextResponse.json({ error: parsed.error.format() }, { status: 400 });
  }
  return adminProxy(auth, {
    method: 'POST',
    path: `/admin/users/${encodeURIComponent(userId)}/passkeys/${encodeURIComponent(passkeyId)}/rename`,
    body: parsed.data,
    idempotencyKey: extractIdempotencyKey(req),
  });
}
