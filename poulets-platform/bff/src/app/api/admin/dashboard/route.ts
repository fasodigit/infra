// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * GET /api/admin/dashboard
 *
 * Aggrège les KPIs admin : utilisateurs actifs 7j, OTP émis 24h, sessions
 * actives, alertes non acquittées, points pour le chart 7j, santé services.
 *
 * Niveau requis : MANAGER (lecture).
 */

import type { NextRequest } from 'next/server';

import { adminAuth } from '@/lib/admin-auth';
import { adminProxy } from '@/lib/admin-proxy';

export async function GET(req: NextRequest) {
  const auth = await adminAuth(req, { requiredLevel: 'MANAGER' });
  if (!auth.ok) return auth.response;
  return adminProxy(auth, {
    method: 'GET',
    path: '/admin/dashboard',
  });
}
