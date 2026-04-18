// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { NextRequest, NextResponse } from 'next/server';
import { impressionClient } from '@/lib/impression-client';

/**
 * POST /api/verification/qr/:code
 * Vérifie l'authenticité d'un QR code (archive WORM).
 */
export async function POST(
  req: NextRequest,
  { params }: { params: Promise<{ code: string }> },
): Promise<NextResponse> {
  try {
    const { code } = await params;
    const jwt = req.cookies.get('ory_kratos_session')?.value;
    const upstream = await impressionClient.verifyQr(code, jwt);
    if (!upstream.ok) {
      return NextResponse.json({ error: upstream.statusText }, { status: upstream.status });
    }
    const data = await upstream.json();
    return NextResponse.json(data);
  } catch (err: any) {
    return NextResponse.json({ error: err?.message ?? 'qr verify failed' }, { status: 500 });
  }
}
