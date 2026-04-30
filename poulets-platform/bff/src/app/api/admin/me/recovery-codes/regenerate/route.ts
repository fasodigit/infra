// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * POST /api/admin/me/recovery-codes/regenerate
 * → self-service regeneration of personal recovery codes (10 single-use).
 *   Codes returned in clear UNE seule fois.
 *
 * Self-management : tout admin authentifié peut régénérer ses propres codes.
 *
 * Référence : DELTA-REQUIREMENTS-2026-04-30 §3.3.
 */

import { NextResponse, type NextRequest } from 'next/server';

import { adminAuth } from '@/lib/admin-auth';
import { auditLog } from '@/lib/admin-audit';
import { adminProxy } from '@/lib/admin-proxy';
import { extractIdempotencyKey } from '@/lib/admin-otp';
import { RecoveryCodeGenerateSchema } from '@/lib/schemas/admin';

export async function POST(req: NextRequest) {
  const auth = await adminAuth(req, { requiredLevel: 'MANAGER' });
  if (!auth.ok) return auth.response;
  const parsed = RecoveryCodeGenerateSchema.safeParse(await req.json().catch(() => ({})));
  if (!parsed.success) {
    return NextResponse.json({ error: parsed.error.format() }, { status: 400 });
  }
  const idempotencyKey = extractIdempotencyKey(req);
  const res = await adminProxy(auth, {
    method: 'POST',
    target: 'auth-ms',
    path: '/admin/me/recovery-codes/regenerate',
    body: parsed.data,
    idempotencyKey,
  });
  if (res.status >= 200 && res.status < 300) {
    void auditLog(
      {
        action: 'RECOVERY_CODES_GENERATED',
        actor: { userId: auth.userId, email: auth.email, role: auth.rawRole, ip: auth.ip },
        target: { type: 'user', id: auth.userId },
        metadata: { motif: parsed.data.motif, self: true },
        traceId: auth.traceparent,
        critical: true,
      },
      { authToken: auth.jwt, idempotencyKey },
    );
  }
  return res;
}
