// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { NextRequest, NextResponse } from 'next/server';
import { impressionClient, ImpressionGenerateInput } from '@/lib/impression-client';

/**
 * POST /api/impression/generate
 * Body : { type, documentId, data }
 * Déclenche un job d'impression → impression-service.
 */
export async function POST(req: NextRequest): Promise<NextResponse> {
  try {
    const body = (await req.json()) as ImpressionGenerateInput;
    if (!body?.type || !body?.documentId) {
      return NextResponse.json({ error: 'type and documentId required' }, { status: 400 });
    }

    const jwt = req.cookies.get('ory_kratos_session')?.value;
    const upstream = await impressionClient.generate(body, jwt);

    if (!upstream.ok) {
      const text = await upstream.text();
      return NextResponse.json({ error: text || upstream.statusText }, { status: upstream.status });
    }

    const data = await upstream.json();
    return NextResponse.json(data);
  } catch (err: any) {
    return NextResponse.json(
      { error: err?.message ?? 'impression generate failed' },
      { status: 500 },
    );
  }
}
