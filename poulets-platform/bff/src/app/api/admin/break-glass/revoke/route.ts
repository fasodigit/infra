// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * POST /api/admin/break-glass/revoke
 * → révoque manuellement une élévation avant son TTL.
 *
 * Niveau requis : SUPER-ADMIN (annulation d'urgence).
 */

import { NextResponse, type NextRequest } from 'next/server';

import { adminAuth } from '@/lib/admin-auth';
import { auditLog } from '@/lib/admin-audit';
import { adminProxy } from '@/lib/admin-proxy';
import { extractIdempotencyKey } from '@/lib/admin-otp';
import { BreakGlassRevokeSchema } from '@/lib/schemas/admin';

export async function POST(req: NextRequest) {
  const auth = await adminAuth(req, { requiredLevel: 'SUPER-ADMIN' });
  if (!auth.ok) return auth.response;
  const parsed = BreakGlassRevokeSchema.safeParse(await req.json().catch(() => ({})));
  if (!parsed.success) {
    return NextResponse.json({ error: parsed.error.format() }, { status: 400 });
  }
  const idempotencyKey = extractIdempotencyKey(req);
  const res = await adminProxy(auth, {
    method: 'POST',
    path: '/admin/break-glass/revoke',
    body: parsed.data,
    idempotencyKey,
  });
  if (res.status >= 200 && res.status < 300) {
    void auditLog(
      {
        action: 'BREAK_GLASS_EXPIRED',
        actor: { userId: auth.userId, email: auth.email, role: auth.rawRole, ip: auth.ip },
        metadata: { motif: parsed.data.motif, manual: true },
        traceId: auth.traceparent,
        critical: true,
      },
      { authToken: auth.jwt, idempotencyKey },
    );
  }
  return res;
}
