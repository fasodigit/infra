import { NextRequest, NextResponse } from 'next/server';
import { mapKratosSession } from '@/lib/kratos';

const KRATOS_PUBLIC_URL = process.env.KRATOS_PUBLIC_URL || 'http://localhost:4433';

/**
 * POST /api/auth/login
 * Login endpoint that proxies to Kratos native login flow.
 * Uses direct HTTP calls instead of @ory/client for reliability.
 *
 * Body: { email: string, password: string }
 * Response: UserSession on success, error on failure.
 * Sets httpOnly session cookie from Kratos.
 */
export async function POST(request: NextRequest) {
  try {
    const { email, password } = await request.json();

    if (!email || !password) {
      return NextResponse.json(
        { error: 'Email and password are required' },
        { status: 400 },
      );
    }

    // Step 1: Create a native login flow
    const flowRes = await fetch(`${KRATOS_PUBLIC_URL}/self-service/login/api`, {
      method: 'GET',
    });
    if (!flowRes.ok) {
      console.error('[BFF] Failed to create login flow:', flowRes.status);
      return NextResponse.json(
        { error: 'Authentication service unavailable' },
        { status: 503 },
      );
    }
    const flow = await flowRes.json();

    // Step 2: Submit credentials to Kratos
    const loginRes = await fetch(
      `${KRATOS_PUBLIC_URL}/self-service/login?flow=${flow.id}`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          method: 'password',
          identifier: email,
          password,
        }),
      },
    );

    const loginData = await loginRes.json();

    if (!loginRes.ok) {
      // Collect error messages
      const nodeMessages = (loginData?.ui?.nodes || []).flatMap(
        (n: any) => (n.messages || []).map((m: any) => m.text),
      );
      const topMessages = (loginData?.ui?.messages || []).map((m: any) => m.text);
      const allMessages = [...topMessages, ...nodeMessages].filter(Boolean);

      console.error('[BFF] Login failed:', loginRes.status, allMessages);

      if (loginRes.status === 400 || loginRes.status === 401) {
        return NextResponse.json(
          { error: allMessages[0] || 'Invalid email or password' },
          { status: 401 },
        );
      }

      return NextResponse.json(
        { error: 'Authentication failed' },
        { status: loginRes.status },
      );
    }

    if (!loginData.session) {
      return NextResponse.json(
        { error: 'Login failed: no session returned' },
        { status: 401 },
      );
    }

    // Build response with user session data
    const userSession = mapKratosSession(loginData.session);
    const response = NextResponse.json(userSession);

    // Forward session token as cookie
    const sessionToken = loginData.session_token;
    if (sessionToken) {
      response.cookies.set('ory_kratos_session', sessionToken, {
        httpOnly: true,
        sameSite: 'lax',
        path: '/',
        domain: 'localhost',
      });
    }

    return response;
  } catch (error: any) {
    console.error('[BFF] Login error:', error?.message);
    return NextResponse.json(
      { error: 'Authentication service unavailable' },
      { status: 503 },
    );
  }
}
