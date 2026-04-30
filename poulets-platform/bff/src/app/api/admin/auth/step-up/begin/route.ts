// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * POST /api/admin/auth/step-up/begin
 *
 * Ouvre une session step-up pour l'utilisateur authentifié. Renvoie
 * `{ sessionId, allowedMethods, expiresAt }` pour permettre au frontend
 * d'afficher le modal `<faso-step-up-guard>` avec les méthodes disponibles.
 *
 * Référence : SECURITY-HARDENING-PLAN-2026-04-30 §4 Tier 4 + §6 Phase 4.b.7.
 */

import { NextResponse, type NextRequest } from 'next/server';

import { adminAuth } from '@/lib/admin-auth';
import { adminProxy } from '@/lib/admin-proxy';
import { extractIdempotencyKey } from '@/lib/admin-otp';
import { StepUpBeginSchema } from '@/lib/schemas/admin';

export async function POST(req: NextRequest) {
  const auth = await adminAuth(req, { requiredLevel: 'MANAGER' });
  if (!auth.ok) return auth.response;

  const parsed = StepUpBeginSchema.safeParse(await req.json().catch(() => ({})));
  if (!parsed.success) {
    return NextResponse.json({ error: parsed.error.format() }, { status: 400 });
  }
  return adminProxy(auth, {
    method: 'POST',
    target: 'auth-ms',
    path: '/admin/auth/step-up/begin',
    body: parsed.data,
    idempotencyKey: extractIdempotencyKey(req),
  });
}
