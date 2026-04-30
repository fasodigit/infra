// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * POST /api/admin/capabilities/check-uniqueness
 *
 * Vérifie qu'un set de capabilities est unique vis-à-vis du rôle ciblé
 * (anti-doublon de privilèges). Utilisé en pré-validation par le frontend
 * lors du flow `grant-role`.
 *
 * Niveau requis : ADMIN.
 *
 * Référence : DELTA-REQUIREMENTS-2026-04-30 §3.4.
 */

import { NextResponse, type NextRequest } from 'next/server';

import { adminAuth } from '@/lib/admin-auth';
import { adminProxy } from '@/lib/admin-proxy';
import { extractIdempotencyKey } from '@/lib/admin-otp';
import { CapabilityCheckUniquenessSchema } from '@/lib/schemas/admin';

export async function POST(req: NextRequest) {
  const auth = await adminAuth(req, { requiredLevel: 'ADMIN' });
  if (!auth.ok) return auth.response;
  const parsed = CapabilityCheckUniquenessSchema.safeParse(
    await req.json().catch(() => ({})),
  );
  if (!parsed.success) {
    return NextResponse.json({ error: parsed.error.format() }, { status: 400 });
  }
  return adminProxy(auth, {
    method: 'POST',
    target: 'auth-ms',
    path: '/admin/capabilities/check-uniqueness',
    body: parsed.data,
    idempotencyKey: extractIdempotencyKey(req),
  });
}
