// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * POST /api/admin/auth/login/risk
 *
 * **PUBLIC** — invoked by the admin login flow after Kratos verifies the
 * password and BEFORE the MFA challenge is shown.
 *
 * Body : `{ email }` → upstream `auth-ms /admin/auth/login/risk`.
 *
 * Response :
 *   200 { decision: "ALLOW" | "STEP_UP", score, signals[] }
 *   403 { decision: "BLOCK", reason }
 *
 * The frontend uses the `decision` to:
 *   - ALLOW   → continue normal MFA flow (Kratos device-trust path).
 *   - STEP_UP → force MFA prompt even if the device is trusted (Tier 5
 *               scoring overrides the trusted-device bypass).
 *   - BLOCK   → render generic refusal page + the user receives an email
 *               via notifier-ms (consumes auth.risk.assessed / .blocked).
 *
 * Référence : SECURITY-HARDENING-PLAN-2026-04-30 §4 Tier 5.
 */

import { NextResponse, type NextRequest } from 'next/server';

import { adminAuth } from '@/lib/admin-auth';
import { adminProxy } from '@/lib/admin-proxy';
import { extractIdempotencyKey } from '@/lib/admin-otp';
import { clientIpFromHeaders, rateLimitCheck } from '@/lib/admin-rate-limit';
import { LoginRiskAssessSchema } from '@/lib/schemas/admin';

const RL_MAX = 30;
const RL_WINDOW_MS = 5 * 60 * 1000;

export async function POST(req: NextRequest) {
  const ip = clientIpFromHeaders(req.headers);
  const rl = rateLimitCheck(`login:risk:${ip}`, RL_MAX, RL_WINDOW_MS);
  if (!rl.allowed) {
    return NextResponse.json(
      { error: 'rate_limit_exceeded' },
      {
        status: 429,
        headers: {
          'Retry-After': String(Math.ceil((rl.resetAt - Date.now()) / 1000)),
        },
      },
    );
  }

  // Public — risk scoring runs PRE-MFA, no Kratos session yet.
  const auth = await adminAuth(req, { allowPublic: true });
  if (!auth.ok) return auth.response;

  const parsed = LoginRiskAssessSchema.safeParse(await req.json().catch(() => ({})));
  if (!parsed.success) {
    return NextResponse.json({ error: parsed.error.format() }, { status: 400 });
  }

  return adminProxy(auth, {
    method: 'POST',
    target: 'auth-ms',
    path: '/admin/auth/login/risk',
    body: parsed.data,
    idempotencyKey: extractIdempotencyKey(req),
  });
}
