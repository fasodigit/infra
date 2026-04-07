import { NextRequest, NextResponse } from 'next/server';
import { kratosFrontend, mapKratosSession, extractCookies } from '@/lib/kratos';

const kratosPublicUrl = process.env.KRATOS_PUBLIC_URL || 'http://localhost:4433';

/**
 * GET /api/auth/session
 * Check if the user has an active Kratos session.
 * Returns the mapped user session or 401.
 */
export async function GET(request: NextRequest) {
  const { searchParams } = new URL(request.url);
  const action = searchParams.get('action');

  // Handle session check (default GET behavior)
  const cookie = extractCookies(request.headers);
  if (!cookie) {
    return NextResponse.json({ error: 'No session' }, { status: 401 });
  }

  try {
    const { data: session } = await kratosFrontend.toSession({
      cookie,
    });

    if (!session?.active) {
      return NextResponse.json({ error: 'Session inactive' }, { status: 401 });
    }

    return NextResponse.json(mapKratosSession(session));
  } catch (error: any) {
    if (error?.response?.status === 401) {
      return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });
    }
    console.error('[BFF] Session check error:', error?.message);
    return NextResponse.json(
      { error: 'Session check failed' },
      { status: 500 },
    );
  }
}

/**
 * POST /api/auth
 * Handles login, register, and logout by proxying to Kratos.
 *
 * Body: { action: 'login' | 'register' | 'logout', ...payload }
 */
export async function POST(request: NextRequest) {
  const body = await request.json();
  const { action, ...payload } = body;

  switch (action) {
    case 'login':
      return handleLogin(request, payload);
    case 'register':
      return handleRegister(request, payload);
    case 'logout':
      return handleLogout(request);
    default:
      return NextResponse.json(
        { error: `Unknown action: ${action}` },
        { status: 400 },
      );
  }
}

// ---------------------------------------------------------------------------
// Login flow
// ---------------------------------------------------------------------------
async function handleLogin(
  request: NextRequest,
  payload: { email: string; password: string },
) {
  try {
    // Step 1: Initialize a login flow
    const { data: flow } = await kratosFrontend.createNativeLoginFlow();

    // Step 2: Submit the login form to Kratos
    const { data: loginResult, headers: responseHeaders } =
      await kratosFrontend.updateLoginFlow({
        flow: flow.id,
        updateLoginFlowBody: {
          method: 'password',
          identifier: payload.email,
          password: payload.password,
        },
      });

    // Step 3: Extract and forward session cookies
    const response = NextResponse.json(mapKratosSession(loginResult.session));

    // Forward Set-Cookie headers from Kratos
    const setCookies = responseHeaders?.['set-cookie'];
    if (setCookies) {
      const cookies = Array.isArray(setCookies) ? setCookies : [setCookies];
      for (const cookie of cookies) {
        response.headers.append('Set-Cookie', cookie);
      }
    }

    return response;
  } catch (error: any) {
    const status = error?.response?.status || 500;
    const kratosError = error?.response?.data;

    if (status === 400 || status === 401) {
      return NextResponse.json(
        {
          error: 'Invalid credentials',
          details: kratosError?.ui?.messages,
        },
        { status: 401 },
      );
    }

    console.error('[BFF] Login error:', error?.message);
    return NextResponse.json({ error: 'Login failed' }, { status: 500 });
  }
}

// ---------------------------------------------------------------------------
// Registration flow
// ---------------------------------------------------------------------------
async function handleRegister(
  request: NextRequest,
  payload: { email: string; password: string; name: string; role: string; phone?: string },
) {
  try {
    // Step 1: Initialize a registration flow
    const { data: flow } = await kratosFrontend.createNativeRegistrationFlow();

    // Step 2: Submit the registration form to Kratos
    const { data: regResult, headers: responseHeaders } =
      await kratosFrontend.updateRegistrationFlow({
        flow: flow.id,
        updateRegistrationFlowBody: {
          method: 'password',
          password: payload.password,
          traits: {
            email: payload.email,
            name: payload.name,
            role: payload.role,
            phone: payload.phone,
          },
        },
      });

    const response = NextResponse.json(
      mapKratosSession(regResult.session || { identity: regResult.identity }),
      { status: 201 },
    );

    // Forward Set-Cookie headers from Kratos
    const setCookies = responseHeaders?.['set-cookie'];
    if (setCookies) {
      const cookies = Array.isArray(setCookies) ? setCookies : [setCookies];
      for (const cookie of cookies) {
        response.headers.append('Set-Cookie', cookie);
      }
    }

    return response;
  } catch (error: any) {
    const status = error?.response?.status || 500;
    const kratosError = error?.response?.data;

    if (status === 400) {
      // Check for duplicate email
      const messages = kratosError?.ui?.messages || [];
      const isDuplicate = messages.some(
        (m: any) => m.id === 4000007 || m.text?.includes('already exists'),
      );

      if (isDuplicate) {
        return NextResponse.json(
          { error: 'Email already registered' },
          { status: 409 },
        );
      }

      return NextResponse.json(
        { error: 'Registration failed', details: messages },
        { status: 400 },
      );
    }

    console.error('[BFF] Registration error:', error?.message);
    return NextResponse.json(
      { error: 'Registration failed' },
      { status: 500 },
    );
  }
}

// ---------------------------------------------------------------------------
// Logout flow
// ---------------------------------------------------------------------------
async function handleLogout(request: NextRequest) {
  const cookie = extractCookies(request.headers);

  try {
    // Create a browser logout flow to get the token
    const { data: logoutFlow } = await kratosFrontend.createBrowserLogoutFlow({
      cookie,
    });

    // Finalize the logout using the token
    await kratosFrontend.updateLogoutFlow({
      token: logoutFlow.logout_token,
    });

    // Clear session cookies
    const response = NextResponse.json({ success: true });
    response.cookies.set('ory_kratos_session', '', {
      maxAge: 0,
      path: '/',
    });

    return response;
  } catch (error: any) {
    console.error('[BFF] Logout error:', error?.message);
    // Even if logout fails, clear the cookie on the client
    const response = NextResponse.json({ success: true });
    response.cookies.set('ory_kratos_session', '', {
      maxAge: 0,
      path: '/',
    });
    return response;
  }
}
