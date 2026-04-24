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
 * Body: { provider, phone, amount }
 *
 * **Authentication required** (enforced by `middleware.ts`). The
 * `X-User-Id` and `X-Tenant-Id` headers are injected by the middleware
 * from the validated Kratos session and MUST be present when we reach
 * this handler. The `reference` field is derived server-side to prevent
 * an attacker from hijacking another user's order identifier (previous
 * design let the client submit any `reference`, which enabled HIGH-
 * severity abuse — fixed 2026-04-20).
 *
 * Behaviour:
 *  - If `MOMO_GATEWAY_URL` env var is set, proxies the request to the
 *    real aggregator (Orange / Moov / Wave) and returns its response.
 *  - Otherwise runs in local-stub mode (dev/e2e smokes).
 *
 * TODO(security) — SMS deep-link payment UX: reintroduce the
 * unauthenticated entry point via HMAC-signed single-use intent tokens
 * issued at order creation and verified here. Do NOT re-open this route
 * to anonymous traffic.
 */

const SUPPORTED_PROVIDERS = new Set(['orange_money', 'moov_africa', 'wave']);
const MOMO_GATEWAY_URL = process.env['MOMO_GATEWAY_URL'];

// Reject phones that are obviously not Burkina Faso MSISDNs. This is a
// defence-in-depth check, not a replacement for provider-side validation.
const BF_PHONE_REGEX = /^\+?226\d{8}$|^\d{8}$/;

interface MoMoBody {
  provider?: string;
  phone?: string;
  amount?: number;
  // Intentionally NO `reference` field — derived server-side.
}

export async function POST(request: NextRequest) {
  // Middleware guarantees these headers are present when we reach here
  // because `/api/payments/mobile-money` is not in `publicPaths`. If
  // somehow they are missing, fail closed.
  const userId = request.headers.get('x-user-id');
  const tenantId = request.headers.get('x-tenant-id') || 'default';
  if (!userId) {
    return NextResponse.json(
      { error: 'Authentication required' },
      { status: 401 },
    );
  }

  let body: MoMoBody;
  try {
    body = (await request.json()) as MoMoBody;
  } catch {
    return NextResponse.json(
      { error: 'Invalid JSON body' },
      { status: 400 },
    );
  }

  const { provider, phone, amount } = body;

  if (!provider || !SUPPORTED_PROVIDERS.has(provider)) {
    return NextResponse.json(
      {
        error:
          `Unsupported provider. Expected one of: ${Array.from(SUPPORTED_PROVIDERS).join(', ')}`,
      },
      { status: 400 },
    );
  }
  if (!phone || typeof phone !== 'string' || !BF_PHONE_REGEX.test(phone)) {
    return NextResponse.json(
      { error: 'Phone number required (BF MSISDN format: +226XXXXXXXX or 8 digits)' },
      { status: 400 },
    );
  }
  if (!amount || typeof amount !== 'number' || amount <= 0) {
    return NextResponse.json({ error: 'Amount must be > 0 FCFA' }, { status: 400 });
  }

  // Server-derived reference. Binding the reference to the authenticated
  // user + tenant prevents an attacker from submitting another user's
  // order-id and tampering with that user's checkout attribution.
  const txSuffix = `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
  const txId = `momo-${txSuffix}`;
  const reference = `order-${tenantId}-${userId}-${txSuffix}`;
  const pollUrl = `/api/payments/mobile-money/status/${txId}`;

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

  return NextResponse.json(
    {
      status: 'PENDING',
      txId,
      pollUrl,
      provider,
      message: `Paiement ${provider} initié (stub local, ${amount} FCFA)`,
    },
    { status: 200 },
  );
}
