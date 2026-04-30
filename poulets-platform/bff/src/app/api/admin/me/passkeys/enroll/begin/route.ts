// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * POST /api/admin/me/passkeys/enroll/begin
 * → self-service WebAuthn enrollment (creation challenge).
 *
 * Self-management : tout admin authentifié peut enroller son propre passkey.
 *
 * Référence : DELTA-REQUIREMENTS-2026-04-30 §3.2.
 */

import { NextResponse, type NextRequest } from 'next/server';

import { adminAuth } from '@/lib/admin-auth';
import { adminProxy } from '@/lib/admin-proxy';
import { extractIdempotencyKey } from '@/lib/admin-otp';
import { WebAuthnRegisterBeginSchema } from '@/lib/schemas/admin';

export async function POST(req: NextRequest) {
  const auth = await adminAuth(req, { requiredLevel: 'MANAGER' });
  if (!auth.ok) return auth.response;
  const raw = await req.json().catch(() => undefined);
  const parsed = WebAuthnRegisterBeginSchema.safeParse(raw);
  if (!parsed.success) {
    return NextResponse.json({ error: parsed.error.format() }, { status: 400 });
  }
  return adminProxy(auth, {
    method: 'POST',
    target: 'auth-ms',
    path: '/admin/me/passkeys/enroll/begin',
    body: parsed.data ?? {},
    idempotencyKey: extractIdempotencyKey(req),
  });
}
