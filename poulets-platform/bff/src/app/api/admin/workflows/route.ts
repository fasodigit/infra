// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { NextRequest, NextResponse } from 'next/server';
import { temporalClient } from '@/lib/temporal-client';

/**
 * GET /api/admin/workflows?type=&status=&actorId=
 * Liste les workflows Temporal (admin-only — enforcement côté poulets-api).
 */
export async function GET(req: NextRequest): Promise<NextResponse> {
  try {
    const jwt = req.cookies.get('ory_kratos_session')?.value;
    const url = new URL(req.url);
    const upstream = await temporalClient.list({
      type: url.searchParams.get('type') ?? undefined,
      status: url.searchParams.get('status') ?? undefined,
      actorId: url.searchParams.get('actorId') ?? undefined,
    }, jwt);
    if (!upstream.ok) {
      return NextResponse.json({ error: upstream.statusText }, { status: upstream.status });
    }
    const data = await upstream.json();
    return NextResponse.json(data);
  } catch (err: any) {
    return NextResponse.json({ error: err?.message ?? 'workflows list failed' }, { status: 500 });
  }
}
