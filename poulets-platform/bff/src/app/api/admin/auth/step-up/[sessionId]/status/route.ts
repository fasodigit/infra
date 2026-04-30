// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * GET /api/admin/auth/step-up/{sessionId}/status
 *
 * Endpoint de polling pour les flows asynchrones (notamment
 * `push-approval` — l'utilisateur doit valider sur un autre device).
 * Renvoie `{ sessionId, status }` où status ∈ {PENDING, VERIFIED, FAILED}.
 */

import { NextResponse, type NextRequest } from 'next/server';

import { adminAuth } from '@/lib/admin-auth';
import { adminProxy } from '@/lib/admin-proxy';

export async function GET(
  req: NextRequest,
  ctx: { params: Promise<{ sessionId: string }> },
) {
  const auth = await adminAuth(req, { requiredLevel: 'MANAGER' });
  if (!auth.ok) return auth.response;

  const { sessionId } = await ctx.params;
  if (!sessionId || sessionId.length < 8) {
    return NextResponse.json({ error: 'invalid_session_id' }, { status: 400 });
  }

  return adminProxy(auth, {
    method: 'GET',
    target: 'auth-ms',
    path: `/admin/auth/step-up/${encodeURIComponent(sessionId)}/status`,
  });
}
