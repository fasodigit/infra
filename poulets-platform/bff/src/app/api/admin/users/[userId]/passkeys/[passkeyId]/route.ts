// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * DELETE /api/admin/users/:userId/passkeys/:passkeyId
 * → supprime un passkey enrollé.
 *
 * Niveau requis : ADMIN.
 */

import type { NextRequest } from 'next/server';

import { adminAuth } from '@/lib/admin-auth';
import { auditLog } from '@/lib/admin-audit';
import { adminProxy } from '@/lib/admin-proxy';
import { extractIdempotencyKey } from '@/lib/admin-otp';

export async function DELETE(
  req: NextRequest,
  { params }: { params: Promise<{ userId: string; passkeyId: string }> },
) {
  const auth = await adminAuth(req, { requiredLevel: 'ADMIN' });
  if (!auth.ok) return auth.response;
  const { userId, passkeyId } = await params;
  const idempotencyKey = extractIdempotencyKey(req);
  const res = await adminProxy(auth, {
    method: 'DELETE',
    path: `/admin/users/${encodeURIComponent(userId)}/passkeys/${encodeURIComponent(passkeyId)}`,
    idempotencyKey,
  });
  if (res.status >= 200 && res.status < 300) {
    void auditLog(
      {
        action: 'MFA_REMOVED',
        actor: { userId: auth.userId, email: auth.email, role: auth.rawRole, ip: auth.ip },
        target: { type: 'passkey', id: passkeyId, label: userId },
        metadata: { method: 'passkey' },
        traceId: auth.traceparent,
      },
      { authToken: auth.jwt, idempotencyKey },
    );
  }
  return res;
}
