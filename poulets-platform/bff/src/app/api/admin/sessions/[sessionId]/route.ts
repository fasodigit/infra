// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * DELETE /api/admin/sessions/:sessionId — révoque une session précise.
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
  { params }: { params: Promise<{ sessionId: string }> },
) {
  const auth = await adminAuth(req, { requiredLevel: 'ADMIN' });
  if (!auth.ok) return auth.response;
  const { sessionId } = await params;
  const idempotencyKey = extractIdempotencyKey(req);
  const res = await adminProxy(auth, {
    method: 'DELETE',
    path: `/admin/sessions/${encodeURIComponent(sessionId)}`,
    idempotencyKey,
  });
  if (res.status >= 200 && res.status < 300) {
    void auditLog(
      {
        action: 'SESSION_REVOKED',
        actor: { userId: auth.userId, email: auth.email, role: auth.rawRole, ip: auth.ip },
        target: { type: 'session', id: sessionId },
        traceId: auth.traceparent,
      },
      { authToken: auth.jwt, idempotencyKey },
    );
  }
  return res;
}
