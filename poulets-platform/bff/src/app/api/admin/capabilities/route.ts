// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * GET /api/admin/capabilities
 *
 * Liste le registry des capabilities granulaires (toutes catégories) défini
 * côté auth-ms. Utilisé par le frontend pour afficher les choix dans le flow
 * `grant-role` (delta §3.4).
 *
 * Niveau requis : MANAGER (lecture).
 *
 * Référence : DELTA-REQUIREMENTS-2026-04-30 §3.4.
 */

import type { NextRequest } from 'next/server';

import { adminAuth } from '@/lib/admin-auth';
import { adminProxy } from '@/lib/admin-proxy';

export async function GET(req: NextRequest) {
  const auth = await adminAuth(req, { requiredLevel: 'MANAGER' });
  if (!auth.ok) return auth.response;
  const url = new URL(req.url);
  return adminProxy(auth, {
    method: 'GET',
    target: 'auth-ms',
    path: '/admin/capabilities',
    query: url.searchParams,
  });
}
