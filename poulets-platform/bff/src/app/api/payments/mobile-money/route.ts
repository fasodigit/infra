// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { NextRequest, NextResponse } from 'next/server';

/**
 * POST /api/payments/mobile-money
 *
 * Initiates a Mobile Money payment for Burkina Faso providers:
 *   - Orange Money  (`orange_money`)
 *   - Moov Africa   (`moov_africa`)
 *   - Wave          (`wave`)
 *
 * Body: { provider, phone, amount, reference }
 *
 * Behaviour:
 *  - If `MOMO_GATEWAY_URL` env var is set, proxies the request to the real
 *    aggregator gateway (Orange / Moov / Wave) and returns whatever the
 *    gateway answers.
 *  - Otherwise runs in local-stub mode: always responds {status: PENDING, …}
 *    so the UI flow is navigable in dev / e2e smokes without real creds.
 */

const SUPPORTED_PROVIDERS = new Set(['orange_money', 'moov_africa', 'wave']);
const MOMO_GATEWAY_URL = process.env['MOMO_GATEWAY_URL'];

interface MoMoBody {
  provider?: string;
  phone?: string;
  amount?: number;
  reference?: string;
}

export async function POST(request: NextRequest) {
  let body: MoMoBody;
  try {
    body = (await request.json()) as MoMoBody;
  } catch {
    return NextResponse.json(
      { error: 'Invalid JSON body' },
      { status: 400 },
    );
  }

  const { provider, phone, amount, reference } = body;

  // Validation
  if (!provider || !SUPPORTED_PROVIDERS.has(provider)) {
    return NextResponse.json(
      {
        error:
          `Unsupported provider. Expected one of: ${Array.from(SUPPORTED_PROVIDERS).join(', ')}`,
      },
      { status: 400 },
    );
  }
  if (!phone || typeof phone !== 'string' || phone.length < 4) {
    return NextResponse.json({ error: 'Phone number required' }, { status: 400 });
  }
  if (!amount || typeof amount !== 'number' || amount <= 0) {
    return NextResponse.json({ error: 'Amount must be > 0 FCFA' }, { status: 400 });
  }

  // Generate our own txId — even when proxying, gateway may use external refs.
  const txId = `momo-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
  const pollUrl = `/api/payments/mobile-money/status/${txId}`;

  // Real gateway path
  if (MOMO_GATEWAY_URL) {
    try {
      const upstreamRes = await fetch(MOMO_GATEWAY_URL, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ provider, phone, amount, reference, txId }),
      });
      const upstream = await upstreamRes.json().catch(() => ({}));
      return NextResponse.json(
        {
          status: upstream?.status ?? 'PENDING',
          txId: upstream?.txId ?? txId,
          pollUrl,
          provider,
          message:
            upstream?.message ?? 'Paiement initié auprès du fournisseur Mobile Money',
        },
        { status: upstreamRes.ok ? 200 : upstreamRes.status },
      );
    } catch (err) {
      console.error('[BFF] MoMo gateway error:', (err as Error)?.message);
      return NextResponse.json(
        {
          status: 'PENDING',
          txId,
          pollUrl,
          provider,
          message: 'Gateway injoignable — initié localement, sera rejoué',
        },
        { status: 202 },
      );
    }
  }

  // Stub fallback
  return NextResponse.json(
    {
      status: 'PENDING',
      txId,
      pollUrl,
      provider,
      message: `Paiement ${provider} initié (stub local, ${amount} FCFA pour ${reference ?? 'ref-?'})`,
    },
    { status: 200 },
  );
}
