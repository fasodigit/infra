// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * POST /api/admin/settings/:key/revert
 * → restaure une version antérieure (publie un nouvel évènement).
 *
 * Niveau requis : SUPER-ADMIN.
 */

import { NextResponse, type NextRequest } from 'next/server';

import { adminAuth } from '@/lib/admin-auth';
import { auditLog } from '@/lib/admin-audit';
import { adminProxy } from '@/lib/admin-proxy';
import { extractIdempotencyKey } from '@/lib/admin-otp';
import { SettingRevertSchema } from '@/lib/schemas/admin';

export async function POST(
  req: NextRequest,
  { params }: { params: Promise<{ key: string }> },
) {
  const auth = await adminAuth(req, { requiredLevel: 'SUPER-ADMIN' });
  if (!auth.ok) return auth.response;
  const { key } = await params;
  const parsed = SettingRevertSchema.safeParse(await req.json().catch(() => ({})));
  if (!parsed.success) {
    return NextResponse.json({ error: parsed.error.format() }, { status: 400 });
  }
  const idempotencyKey = extractIdempotencyKey(req);
  const res = await adminProxy(auth, {
    method: 'POST',
    path: `/admin/settings/${encodeURIComponent(key)}/revert`,
    body: parsed.data,
    idempotencyKey,
  });
  if (res.status >= 200 && res.status < 300) {
    void auditLog(
      {
        action: 'SETTINGS_REVERTED',
        actor: { userId: auth.userId, email: auth.email, role: auth.rawRole, ip: auth.ip },
        target: { type: 'setting', id: key },
        metadata: {
          targetVersion: parsed.data.targetVersion,
          motif: parsed.data.motif,
        },
        traceId: auth.traceparent,
        critical: true,
      },
      { authToken: auth.jwt, idempotencyKey },
    );
  }
  return res;
}
