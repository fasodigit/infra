// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * POST /api/admin/devices/:deviceId/trust — marque un appareil comme trusté.
 *
 * Niveau requis : ADMIN.
 */

import { NextResponse, type NextRequest } from 'next/server';

import { adminAuth } from '@/lib/admin-auth';
import { auditLog } from '@/lib/admin-audit';
import { adminProxy } from '@/lib/admin-proxy';
import { extractIdempotencyKey } from '@/lib/admin-otp';
import { TrustDeviceSchema } from '@/lib/schemas/admin';

export async function POST(
  req: NextRequest,
  { params }: { params: Promise<{ deviceId: string }> },
) {
  const auth = await adminAuth(req, { requiredLevel: 'ADMIN' });
  if (!auth.ok) return auth.response;
  const { deviceId } = await params;
  const raw = await req.json().catch(() => undefined);
  const parsed = TrustDeviceSchema.safeParse(raw);
  if (!parsed.success) {
    return NextResponse.json({ error: parsed.error.format() }, { status: 400 });
  }
  const idempotencyKey = extractIdempotencyKey(req);
  const res = await adminProxy(auth, {
    method: 'POST',
    path: `/admin/devices/${encodeURIComponent(deviceId)}/trust`,
    body: parsed.data ?? {},
    idempotencyKey,
  });
  if (res.status >= 200 && res.status < 300) {
    void auditLog(
      {
        action: 'DEVICE_TRUSTED',
        actor: { userId: auth.userId, email: auth.email, role: auth.rawRole, ip: auth.ip },
        target: { type: 'device', id: deviceId },
        metadata: parsed.data ?? {},
        traceId: auth.traceparent,
      },
      { authToken: auth.jwt, idempotencyKey },
    );
  }
  return res;
}
