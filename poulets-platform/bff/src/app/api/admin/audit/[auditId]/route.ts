// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * GET /api/admin/audit/:auditId — détail d'une entrée audit.
 *
 * Niveau requis : MANAGER.
 */

import type { NextRequest } from 'next/server';

import { adminAuth } from '@/lib/admin-auth';
import { adminProxy } from '@/lib/admin-proxy';

export async function GET(
  req: NextRequest,
  { params }: { params: Promise<{ auditId: string }> },
) {
  const auth = await adminAuth(req, { requiredLevel: 'MANAGER' });
  if (!auth.ok) return auth.response;
  const { auditId } = await params;
  return adminProxy(auth, {
    method: 'GET',
    path: `/admin/audit/${encodeURIComponent(auditId)}`,
  });
}
