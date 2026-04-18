// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { NextRequest, NextResponse } from 'next/server';
import { impressionClient } from '@/lib/impression-client';

/**
 * GET /api/impression/:id
 * Détail d'un job d'impression.
 */
export async function GET(
  req: NextRequest,
  { params }: { params: Promise<{ id: string }> },
): Promise<NextResponse> {
  try {
    const { id } = await params;
    const jwt = req.cookies.get('ory_kratos_session')?.value;
    const upstream = await impressionClient.getJob(id, jwt);
    if (!upstream.ok) {
      return NextResponse.json({ error: upstream.statusText }, { status: upstream.status });
    }
    const data = await upstream.json();
    return NextResponse.json(data);
  } catch (err: any) {
    return NextResponse.json({ error: err?.message ?? 'job fetch failed' }, { status: 500 });
  }
}
