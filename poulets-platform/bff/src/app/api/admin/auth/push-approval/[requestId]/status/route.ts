// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * GET /api/admin/auth/push-approval/[requestId]/status
 *
 * Polling fallback pour les clients qui ne peuvent pas maintenir une connexion
 * WebSocket. Renvoie le statut courant de la demande d'approbation push.
 *
 * Réponse : `{ requestId, status: "PENDING"|"GRANTED"|"DENIED"|"TIMEOUT", granted }`.
 *
 * Requiert MANAGER ou supérieur.
 *
 * Note : Ce polling est un fallback uniquement. Le flow normal utilise le WS
 * `/ws/admin/approval` (Phase 4.b.5).
 */

import { NextResponse, type NextRequest } from 'next/server';

import { adminAuth } from '@/lib/admin-auth';
import { adminProxy } from '@/lib/admin-proxy';

interface RouteParams {
  params: Promise<{ requestId: string }>;
}

export async function GET(req: NextRequest, { params }: RouteParams) {
  const { requestId } = await params;

  if (!requestId || !/^[0-9a-f-]{36}$/i.test(requestId)) {
    return NextResponse.json({ error: 'invalid_request_id' }, { status: 400 });
  }

  const auth = await adminAuth(req, { requiredLevel: 'MANAGER' });
  if (!auth.ok) return auth.response;

  return adminProxy(auth, {
    method: 'GET',
    target: 'auth-ms',
    path: `/admin/auth/push-approval/${requestId}/status`,
  });
}
