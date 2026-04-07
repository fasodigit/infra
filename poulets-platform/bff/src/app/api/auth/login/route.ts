import { NextRequest, NextResponse } from 'next/server';
import { kratosFrontend, mapKratosSession } from '@/lib/kratos';

/**
 * POST /api/auth/login
 * Login endpoint that proxies to Kratos native login flow.
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
    const { data: flow } = await kratosFrontend.createNativeLoginFlow();

    // Step 2: Submit credentials to Kratos
    const { data: loginResult, headers: responseHeaders } =
      await kratosFrontend.updateLoginFlow({
        flow: flow.id,
        updateLoginFlowBody: {
          method: 'password',
          identifier: email,
          password: password,
        },
      });

    if (!loginResult.session) {
      return NextResponse.json(
        { error: 'Login failed: no session returned' },
        { status: 401 },
      );
    }

    // Build response with user session data
    const userSession = mapKratosSession(loginResult.session);
    const response = NextResponse.json(userSession);

    // Forward Kratos session cookies to the client
    const setCookies = responseHeaders?.['set-cookie'];
    if (setCookies) {
      const cookies = Array.isArray(setCookies) ? setCookies : [setCookies];
      for (const cookie of cookies) {
        response.headers.append('Set-Cookie', cookie);
      }
    }

    return response;
  } catch (error: any) {
    const status = error?.response?.status;

    if (status === 400 || status === 401) {
      return NextResponse.json(
        { error: 'Invalid email or password' },
        { status: 401 },
      );
    }

    console.error('[BFF] Login error:', error?.message);
    return NextResponse.json(
      { error: 'Authentication service unavailable' },
      { status: 503 },
    );
  }
}
