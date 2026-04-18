// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { NextRequest, NextResponse } from 'next/server';
import { temporalClient } from '@/lib/temporal-client';

/**
 * POST /api/admin/workflows/:id/signal
 * Body : { name: string, payload?: unknown }
 * Envoie un signal Temporal au workflow en cours.
 */
export async function POST(
  req: NextRequest,
  { params }: { params: Promise<{ id: string }> },
): Promise<NextResponse> {
  try {
    const { id } = await params;
    const body = (await req.json()) as { name: string; payload?: unknown };
    if (!body?.name) {
      return NextResponse.json({ error: 'signal name required' }, { status: 400 });
    }
    const jwt = req.cookies.get('ory_kratos_session')?.value;
    const upstream = await temporalClient.signal(id, body.name, body.payload, jwt);
    if (!upstream.ok) {
      return NextResponse.json({ error: upstream.statusText }, { status: upstream.status });
    }
    return NextResponse.json({ ok: true });
  } catch (err: any) {
    return NextResponse.json({ error: err?.message ?? 'signal failed' }, { status: 500 });
  }
}
