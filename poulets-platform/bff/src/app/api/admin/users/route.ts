// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * GET /api/admin/users           — liste paginée (?role&department&page&size&sort)
 * POST /api/admin/users           — invitation magic-link (body InviteAdminSchema)
 *
 * GET : niveau MANAGER (lecture). POST : niveau ADMIN (invite).
 */

import { NextResponse, type NextRequest } from 'next/server';

import { adminAuth } from '@/lib/admin-auth';
import { auditLog } from '@/lib/admin-audit';
import { adminProxy } from '@/lib/admin-proxy';
import { extractIdempotencyKey } from '@/lib/admin-otp';
import { InviteAdminSchema } from '@/lib/schemas/admin';

export async function GET(req: NextRequest) {
  const auth = await adminAuth(req, { requiredLevel: 'MANAGER' });
  if (!auth.ok) return auth.response;
  const url = new URL(req.url);
  return adminProxy(auth, {
    method: 'GET',
    path: '/admin/users',
    query: url.searchParams,
  });
}

export async function POST(req: NextRequest) {
  const auth = await adminAuth(req, { requiredLevel: 'ADMIN' });
  if (!auth.ok) return auth.response;
  const parsed = InviteAdminSchema.safeParse(await req.json().catch(() => ({})));
  if (!parsed.success) {
    return NextResponse.json({ error: parsed.error.format() }, { status: 400 });
  }
  const idempotencyKey = extractIdempotencyKey(req);
  const res = await adminProxy(auth, {
    method: 'POST',
    path: '/admin/users/invite',
    body: parsed.data,
    idempotencyKey,
  });
  if (res.status >= 200 && res.status < 300) {
    void auditLog(
      {
        action: 'ADMIN_INVITE_SENT',
        actor: { userId: auth.userId, email: auth.email, role: auth.rawRole, ip: auth.ip },
        target: { type: 'user', label: parsed.data.email },
        metadata: { role: parsed.data.role },
        traceId: auth.traceparent,
      },
      { authToken: auth.jwt, idempotencyKey },
    );
  }
  return res;
}
