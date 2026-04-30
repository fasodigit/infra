// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * POST /api/admin/auth/push-approval/initiate
 *
 * Initie une demande d'approbation push (Phase 4.b.5 — sovereign WebSocket MFA).
 *
 * Requiert MANAGER ou supérieur (level MANAGER dans la hiérarchie admin).
 * Le BFF extrait userId + ip + ua du contexte auth et les transmet à auth-ms.
 *
 * Réponse :
 * - `{ available: true, requestId, displayedNumber, expiresAt }` si l'utilisateur
 *   a une session WS active sur /ws/admin/approval.
 * - `{ available: false, fallback: "OTP" }` sinon.
 */

import { NextResponse, type NextRequest } from 'next/server';

import { adminAuth } from '@/lib/admin-auth';
import { adminProxy } from '@/lib/admin-proxy';
import { clientIpFromHeaders } from '@/lib/admin-rate-limit';
import { PushApprovalInitiateSchema } from '@/lib/schemas/admin';

export async function POST(req: NextRequest) {
  const auth = await adminAuth(req, { requiredLevel: 'MANAGER' });
  if (!auth.ok) return auth.response;

  const ip = clientIpFromHeaders(req.headers);
  const ua = req.headers.get('user-agent') ?? '';

  const body = await req.json().catch(() => ({}));

  const parsed = PushApprovalInitiateSchema.safeParse({
    userId: auth.userId,
    ip,
    ua,
    city: (body as Record<string, unknown>).city,
  });

  if (!parsed.success) {
    return NextResponse.json({ error: parsed.error.format() }, { status: 400 });
  }

  return adminProxy(auth, {
    method: 'POST',
    target: 'auth-ms',
    path: '/admin/auth/push-approval/initiate',
    body: parsed.data,
  });
}
