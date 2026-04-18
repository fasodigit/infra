// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { NextRequest, NextResponse } from 'next/server';
import { impressionClient } from '@/lib/impression-client';

/**
 * GET /api/impression/:id/pdf
 * Retourne le PDF binaire (streaming via Blob).
 */
export async function GET(
  req: NextRequest,
  { params }: { params: Promise<{ id: string }> },
): Promise<Response> {
  try {
    const { id } = await params;
    const jwt = req.cookies.get('ory_kratos_session')?.value;
    const upstream = await impressionClient.downloadPdf(id, jwt);
    if (!upstream.ok) {
      return NextResponse.json({ error: upstream.statusText }, { status: upstream.status });
    }
    const headers = new Headers();
    headers.set('Content-Type', upstream.headers.get('Content-Type') ?? 'application/pdf');
    const cd = upstream.headers.get('Content-Disposition');
    if (cd) headers.set('Content-Disposition', cd);
    return new Response(upstream.body, { status: 200, headers });
  } catch (err: any) {
    return NextResponse.json({ error: err?.message ?? 'pdf fetch failed' }, { status: 500 });
  }
}
