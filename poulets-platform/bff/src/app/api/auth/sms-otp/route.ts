// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { NextRequest, NextResponse } from 'next/server';

/**
 * POST /api/auth/sms-otp
 *
 * Sends an OTP code by SMS as fallback to e-mail verification.
 *
 * Body: { phone: string }
 *
 * Behaviour:
 *  - If `SMS_PROVIDER_URL` is set (Orange SMS API or aggregator), proxies
 *    the request upstream.
 *  - Otherwise stubs a `{sent:true, expiresAt}` response for dev / e2e.
 *
 * Future: integrate with Kratos SMS method (self-service/verification flow)
 * once the Kratos SMS courier is configured (see INFRA/ory/kratos.yml).
 */

const OTP_TTL_SEC = 300; // 5 minutes
const SMS_PROVIDER_URL = process.env['SMS_PROVIDER_URL'];

interface SmsOtpBody {
  phone?: string;
}

export async function POST(request: NextRequest) {
  let body: SmsOtpBody;
  try {
    body = (await request.json()) as SmsOtpBody;
  } catch {
    return NextResponse.json({ error: 'Invalid JSON body' }, { status: 400 });
  }

  const phone = body.phone?.trim();
  if (!phone || phone.length < 4) {
    return NextResponse.json(
      { sent: false, error: 'Phone number required' },
      { status: 400 },
    );
  }

  const expiresAt = new Date(Date.now() + OTP_TTL_SEC * 1000).toISOString();

  // Real SMS provider path
  if (SMS_PROVIDER_URL) {
    try {
      const upstreamRes = await fetch(SMS_PROVIDER_URL, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ phone, channel: 'sms', purpose: 'otp' }),
      });
      const upstream = await upstreamRes.json().catch(() => ({}));
      return NextResponse.json(
        {
          sent: upstreamRes.ok && (upstream?.sent ?? true),
          expiresAt: upstream?.expiresAt ?? expiresAt,
          message: upstream?.message ?? 'OTP envoyé par SMS',
        },
        { status: upstreamRes.ok ? 200 : upstreamRes.status },
      );
    } catch (err) {
      console.error('[BFF] SMS provider error:', (err as Error)?.message);
      return NextResponse.json(
        {
          sent: false,
          message: 'Fournisseur SMS injoignable',
        },
        { status: 503 },
      );
    }
  }

  // Stub fallback
  return NextResponse.json(
    {
      sent: true,
      expiresAt,
      message: `OTP simulé envoyé à ${phone} (mode stub)`,
    },
    { status: 200 },
  );
}
