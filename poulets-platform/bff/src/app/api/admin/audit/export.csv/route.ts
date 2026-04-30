// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * POST /api/admin/audit/export.csv
 * → exporte un CSV d'audit selon les filtres (body AuditFiltersSchema).
 *
 * Niveau requis : ADMIN (export sensible — rétention 7 ans Loi 010-2004 BF).
 */

import { NextResponse, type NextRequest } from 'next/server';

import { adminAuth } from '@/lib/admin-auth';
import { auditLog } from '@/lib/admin-audit';
import { adminProxy } from '@/lib/admin-proxy';
import { extractIdempotencyKey } from '@/lib/admin-otp';
import { AuditFiltersSchema } from '@/lib/schemas/admin';

export async function POST(req: NextRequest) {
  const auth = await adminAuth(req, { requiredLevel: 'ADMIN' });
  if (!auth.ok) return auth.response;
  const parsed = AuditFiltersSchema.safeParse(await req.json().catch(() => ({})));
  if (!parsed.success) {
    return NextResponse.json({ error: parsed.error.format() }, { status: 400 });
  }
  const idempotencyKey = extractIdempotencyKey(req);
  const res = await adminProxy(auth, {
    method: 'POST',
    path: '/admin/audit/export.csv',
    body: parsed.data,
    idempotencyKey,
    rawResponse: true,
  });
  void auditLog(
    {
      action: 'AUDIT_EXPORTED',
      actor: { userId: auth.userId, email: auth.email, role: auth.rawRole, ip: auth.ip },
      target: { type: 'audit', label: 'csv-export' },
      metadata: parsed.data as Record<string, unknown>,
      traceId: auth.traceparent,
      critical: true,
    },
    { authToken: auth.jwt, idempotencyKey },
  );
  return res;
}
