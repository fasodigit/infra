// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * POST /api/admin/users/:userId/recovery/initiate
 *
 * Admin-driven recovery : un SUPER-ADMIN déclenche la procédure de
 * récupération pour le compte d'un autre utilisateur (ex : utilisateur
 * a perdu son MFA + n'a plus accès email).
 *
 * Niveau requis : SUPER-ADMIN + Keto check
 * `AdminRole:platform#manage_users@subjectId`.
 *
 * Body : `{ motif (≥50), otpProof (8 digits) }`.
 *
 * Référence : DELTA-REQUIREMENTS-2026-04-30 §5.
 */

import { NextResponse, type NextRequest } from 'next/server';

import { adminAuth } from '@/lib/admin-auth';
import { auditLog } from '@/lib/admin-audit';
import { adminProxy } from '@/lib/admin-proxy';
import { extractIdempotencyKey } from '@/lib/admin-otp';
import { AdminRecoveryInitiateSchema } from '@/lib/schemas/admin';

export async function POST(
  req: NextRequest,
  { params }: { params: Promise<{ userId: string }> },
) {
  const { userId } = await params;
  const auth = await adminAuth(req, {
    requiredLevel: 'SUPER-ADMIN',
    ketoCheck: {
      namespace: 'AdminRole',
      object: 'platform',
      relation: 'manage_users',
    },
  });
  if (!auth.ok) return auth.response;

  const parsed = AdminRecoveryInitiateSchema.safeParse(await req.json().catch(() => ({})));
  if (!parsed.success) {
    return NextResponse.json({ error: parsed.error.format() }, { status: 400 });
  }
  const idempotencyKey = extractIdempotencyKey(req);
  const res = await adminProxy(auth, {
    method: 'POST',
    target: 'auth-ms',
    path: `/admin/users/${encodeURIComponent(userId)}/recovery/initiate`,
    body: parsed.data,
    idempotencyKey,
  });
  if (res.status >= 200 && res.status < 300) {
    void auditLog(
      {
        action: 'ADMIN_RECOVERY_INITIATED',
        actor: { userId: auth.userId, email: auth.email, role: auth.rawRole, ip: auth.ip },
        target: { type: 'user', id: userId },
        metadata: { motif: parsed.data.motif },
        traceId: auth.traceparent,
        critical: true,
      },
      { authToken: auth.jwt, idempotencyKey },
    );
  }
  return res;
}
