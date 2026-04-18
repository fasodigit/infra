// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { NextRequest, NextResponse } from 'next/server';
import { temporalClient } from '@/lib/temporal-client';

/**
 * POST /api/admin/workflows/:id/cancel
 * Body : { reason?: string }
 */
export async function POST(
  req: NextRequest,
  { params }: { params: Promise<{ id: string }> },
): Promise<NextResponse> {
  try {
    const { id } = await params;
    const body = (await req.json().catch(() => ({}))) as { reason?: string };
    const jwt = req.cookies.get('ory_kratos_session')?.value;
    const upstream = await temporalClient.cancel(id, body.reason ?? 'admin-action', jwt);
    if (!upstream.ok) {
      return NextResponse.json({ error: upstream.statusText }, { status: upstream.status });
    }
    return NextResponse.json({ ok: true });
  } catch (err: any) {
    return NextResponse.json({ error: err?.message ?? 'cancel failed' }, { status: 500 });
  }
}
