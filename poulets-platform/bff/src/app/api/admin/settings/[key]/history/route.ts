// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * GET /api/admin/settings/:key/history — historique versions paginé.
 *
 * Niveau requis : MANAGER.
 */

import type { NextRequest } from 'next/server';

import { adminAuth } from '@/lib/admin-auth';
import { adminProxy } from '@/lib/admin-proxy';

export async function GET(
  req: NextRequest,
  { params }: { params: Promise<{ key: string }> },
) {
  const auth = await adminAuth(req, { requiredLevel: 'MANAGER' });
  if (!auth.ok) return auth.response;
  const { key } = await params;
  const url = new URL(req.url);
  return adminProxy(auth, {
    method: 'GET',
    path: `/admin/settings/${encodeURIComponent(key)}/history`,
    query: url.searchParams,
  });
}
