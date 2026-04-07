import { NextRequest, NextResponse } from 'next/server';
import { kratosFrontend, extractCookies } from '@/lib/kratos';

/**
 * POST /api/auth/logout
 * Destroys the Kratos session and clears session cookies.
 */
export async function POST(request: NextRequest) {
  const cookie = extractCookies(request.headers);

  try {
    if (cookie) {
      // Create a browser logout flow to get the token
      const { data: logoutFlow } = await kratosFrontend.createBrowserLogoutFlow({
        cookie,
      });

      // Finalize the logout using the token
      await kratosFrontend.updateLogoutFlow({
        token: logoutFlow.logout_token,
      });
    }
  } catch (error: any) {
    // Log but don't fail - we still want to clear the cookie
    console.error('[BFF] Logout flow error:', error?.message);
  }

  // Always clear the session cookie on the client side
  const response = NextResponse.json({ success: true });
  response.cookies.set('ory_kratos_session', '', {
    maxAge: 0,
    path: '/',
    httpOnly: true,
    sameSite: 'lax',
  });

  return response;
}
