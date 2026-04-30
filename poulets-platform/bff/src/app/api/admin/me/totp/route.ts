// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * DELETE /api/admin/me/totp
 * → self-service TOTP disable.
 *
 * Self-management : tout admin authentifié peut désactiver son propre TOTP.
 *
 * Référence : DELTA-REQUIREMENTS-2026-04-30 §3.2.
 */

import type { NextRequest } from 'next/server';

import { adminAuth } from '@/lib/admin-auth';
import { auditLog } from '@/lib/admin-audit';
import { adminProxy } from '@/lib/admin-proxy';
import { extractIdempotencyKey } from '@/lib/admin-otp';

export async function DELETE(req: NextRequest) {
  const auth = await adminAuth(req, { requiredLevel: 'MANAGER' });
  if (!auth.ok) return auth.response;
  const idempotencyKey = extractIdempotencyKey(req);
  const res = await adminProxy(auth, {
    method: 'DELETE',
    target: 'auth-ms',
    path: '/admin/me/totp',
    idempotencyKey,
  });
  if (res.status >= 200 && res.status < 300) {
    void auditLog(
      {
        action: 'MFA_REMOVED',
        actor: { userId: auth.userId, email: auth.email, role: auth.rawRole, ip: auth.ip },
        target: { type: 'user', id: auth.userId },
        metadata: { method: 'totp', self: true },
        traceId: auth.traceparent,
      },
      { authToken: auth.jwt, idempotencyKey },
    );
  }
  return res;
}
