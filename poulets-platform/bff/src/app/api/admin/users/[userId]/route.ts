// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * GET /api/admin/users/:userId — détail utilisateur (MFA, devices count, etc.)
 *
 * Niveau requis : MANAGER (lecture).
 */

import type { NextRequest } from 'next/server';

import { adminAuth } from '@/lib/admin-auth';
import { adminProxy } from '@/lib/admin-proxy';

export async function GET(
  req: NextRequest,
  { params }: { params: Promise<{ userId: string }> },
) {
  const auth = await adminAuth(req, { requiredLevel: 'MANAGER' });
  if (!auth.ok) return auth.response;
  const { userId } = await params;
  return adminProxy(auth, {
    method: 'GET',
    path: `/admin/users/${encodeURIComponent(userId)}`,
  });
}
