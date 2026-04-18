import { NextRequest, NextResponse } from 'next/server';
import { mapKratosSession } from '@/lib/kratos';

const KRATOS_PUBLIC_URL = process.env.KRATOS_PUBLIC_URL || 'http://localhost:4433';

/**
 * GET /api/auth/session
 * Validates the current Kratos session and returns user data.
 * Called by the Angular frontend on bootstrap to check if user is logged in.
 *
 * Kratos supports two session mechanisms:
 *   1. Browser flow  → encrypted session cookie `ory_kratos_session=<~400 char payload>`.
 *      Validated by forwarding `Cookie` header as-is to Kratos.
 *   2. API flow      → session token `ory_st_<32 char token>` (also stored by the
 *      Angular SPA in the `ory_kratos_session` cookie). Must be forwarded as
 *      `X-Session-Token` header; sending it via `Cookie` returns 401 because
 *      Kratos tries to decrypt it as an encrypted payload.
 *
 * We detect the `ory_st_` prefix to route each session to the right Kratos
 * auth header. When both a Bearer token and a cookie are present we let the
 * Bearer win (API clients override browser state).
 */
const KRATOS_TOKEN_PREFIX = 'ory_st_';

export async function GET(request: NextRequest) {
  const cookieHeader = request.headers.get('cookie') || '';
  const authzHeader = request.headers.get('authorization') || '';
  const kratosSessionCookie = request.cookies.get('ory_kratos_session')?.value;

  const hasCookie = Boolean(
    kratosSessionCookie || /(?:^|;\s*)ory_kratos_session=/.test(cookieHeader),
  );
  const hasBearer = /^Bearer\s+/i.test(authzHeader);

  if (!hasCookie && !hasBearer) {
    return NextResponse.json(
      { error: 'No session cookie present' },
      { status: 401 },
    );
  }

  try {
    // Decide which Kratos auth mechanism to use.
    // NEVER combine Cookie + X-Session-Token/Authorization — Kratos prefers the
    // token header and will reject the encrypted cookie value as invalid, 401.
    const headers: Record<string, string> = {};
    if (hasBearer) {
      headers['Authorization'] = authzHeader;
    } else if (
      kratosSessionCookie &&
      kratosSessionCookie.startsWith(KRATOS_TOKEN_PREFIX)
    ) {
      // The cookie actually holds a Kratos session token (API flow). Forward
      // as X-Session-Token so Kratos performs token auth instead of attempting
      // to decrypt the value as an encrypted cookie payload.
      headers['X-Session-Token'] = kratosSessionCookie;
    } else {
      headers['Cookie'] = cookieHeader;
    }

    const sessionRes = await fetch(`${KRATOS_PUBLIC_URL}/sessions/whoami`, {
      method: 'GET',
      headers,
      redirect: 'manual',
    });

    if (sessionRes.status === 401 || sessionRes.status === 403) {
      return NextResponse.json(
        { error: 'Session expired or invalid' },
        { status: 401 },
      );
    }

    if (!sessionRes.ok) {
      console.error(
        '[BFF session] Kratos upstream error',
        sessionRes.status,
        await sessionRes.text().catch(() => ''),
      );
      return NextResponse.json(
        { error: 'Auth service unavailable' },
        { status: 503 },
      );
    }

    const session = await sessionRes.json();

    if (!session?.active) {
      return NextResponse.json(
        { error: 'Session is not active' },
        { status: 401 },
      );
    }

    return NextResponse.json(mapKratosSession(session));
  } catch (error: any) {
    console.error('[BFF session] Validation error:', error?.message);
    return NextResponse.json(
      { error: 'Session validation failed' },
      { status: 503 },
    );
  }
}
