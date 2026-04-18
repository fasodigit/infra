// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { NextRequest, NextResponse } from 'next/server';
import { impressionClient } from '@/lib/impression-client';

/**
 * GET /api/impression/queue
 * Liste les jobs d'impression (admin only — filtré côté upstream via JWT).
 */
export async function GET(req: NextRequest): Promise<NextResponse> {
  try {
    const jwt = req.cookies.get('ory_kratos_session')?.value;
    const upstream = await impressionClient.listQueue(jwt);
    if (!upstream.ok) {
      return NextResponse.json({ error: upstream.statusText }, { status: upstream.status });
    }
    const data = await upstream.json();
    return NextResponse.json(data);
  } catch (err: any) {
    return NextResponse.json({ error: err?.message ?? 'queue fetch failed' }, { status: 500 });
  }
}
