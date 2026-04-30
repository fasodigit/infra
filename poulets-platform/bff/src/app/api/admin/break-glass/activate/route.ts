// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * POST /api/admin/break-glass/activate
 * → élévation 4h max (capability db|grant|settings).
 *
 * Niveau requis : ADMIN (Keto check `activate_break_glass`). SUPER-ADMIN
 * et ADMIN sont autorisés selon le namespace AdminRole défini en §8.
 */

import { NextResponse, type NextRequest } from 'next/server';

import { adminAuth } from '@/lib/admin-auth';
import { auditLog } from '@/lib/admin-audit';
import { adminProxy } from '@/lib/admin-proxy';
import { extractIdempotencyKey } from '@/lib/admin-otp';
import { BreakGlassRequestSchema } from '@/lib/schemas/admin';

export async function POST(req: NextRequest) {
  const auth = await adminAuth(req, {
    requiredLevel: 'ADMIN',
    ketoCheck: {
      namespace: 'AdminRole',
      object: 'global',
      relation: 'activate_break_glass',
    },
  });
  if (!auth.ok) return auth.response;
  const parsed = BreakGlassRequestSchema.safeParse(await req.json().catch(() => ({})));
  if (!parsed.success) {
    return NextResponse.json({ error: parsed.error.format() }, { status: 400 });
  }
  const idempotencyKey = extractIdempotencyKey(req);
  const res = await adminProxy(auth, {
    method: 'POST',
    path: '/admin/break-glass/activate',
    body: parsed.data,
    idempotencyKey,
  });
  if (res.status >= 200 && res.status < 300) {
    void auditLog(
      {
        action: 'BREAK_GLASS_ACTIVATED',
        actor: { userId: auth.userId, email: auth.email, role: auth.rawRole, ip: auth.ip },
        target: { type: 'capability', id: parsed.data.capability },
        metadata: {
          justification: parsed.data.justification,
          durationSeconds: parsed.data.durationSeconds,
        },
        traceId: auth.traceparent,
        critical: true,
      },
      { authToken: auth.jwt, idempotencyKey },
    );
  }
  return res;
}
