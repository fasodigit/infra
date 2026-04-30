// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * DELETE /api/admin/me/passkeys/:passkeyId
 * → self-service removal of an enrolled passkey.
 *
 * Self-management : tout admin authentifié peut supprimer un de ses passkeys.
 *
 * Référence : DELTA-REQUIREMENTS-2026-04-30 §3.2.
 */

import type { NextRequest } from 'next/server';

import { adminAuth } from '@/lib/admin-auth';
import { auditLog } from '@/lib/admin-audit';
import { adminProxy } from '@/lib/admin-proxy';
import { extractIdempotencyKey } from '@/lib/admin-otp';

export async function DELETE(
  req: NextRequest,
  { params }: { params: Promise<{ passkeyId: string }> },
) {
  const auth = await adminAuth(req, { requiredLevel: 'MANAGER' });
  if (!auth.ok) return auth.response;
  const { passkeyId } = await params;
  const idempotencyKey = extractIdempotencyKey(req);
  const res = await adminProxy(auth, {
    method: 'DELETE',
    target: 'auth-ms',
    path: `/admin/me/passkeys/${encodeURIComponent(passkeyId)}`,
    idempotencyKey,
  });
  if (res.status >= 200 && res.status < 300) {
    void auditLog(
      {
        action: 'MFA_REMOVED',
        actor: { userId: auth.userId, email: auth.email, role: auth.rawRole, ip: auth.ip },
        target: { type: 'passkey', id: passkeyId, label: auth.userId },
        metadata: { method: 'passkey', self: true },
        traceId: auth.traceparent,
      },
      { authToken: auth.jwt, idempotencyKey },
    );
  }
  return res;
}
