import { NextRequest, NextResponse } from 'next/server';
import { mapKratosSession } from '@/lib/kratos';

const KRATOS_PUBLIC_URL = process.env.KRATOS_PUBLIC_URL || 'http://localhost:4433';

/**
 * GET /api/auth/session
 * Validates the current Kratos session and returns user data.
 * Called by the Angular frontend on bootstrap to check if user is logged in.
 */
export async function GET(request: NextRequest) {
  const cookie = request.headers.get('cookie') || '';

  // Check for session token in cookie
  const sessionToken = request.cookies.get('ory_kratos_session')?.value;

  if (!cookie && !sessionToken) {
    return NextResponse.json(
      { error: 'No session cookie present' },
      { status: 401 },
    );
  }

  try {
    // Use Kratos toSession endpoint
    const headers: Record<string, string> = {};
    if (sessionToken) {
      headers['X-Session-Token'] = sessionToken;
    }
    if (cookie) {
      headers['Cookie'] = cookie;
    }

    const sessionRes = await fetch(`${KRATOS_PUBLIC_URL}/sessions/whoami`, {
      method: 'GET',
      headers,
    });

    if (!sessionRes.ok) {
      return NextResponse.json(
        { error: 'Session expired or invalid' },
        { status: 401 },
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
    console.error('[BFF] Session validation error:', error?.message);
    return NextResponse.json(
      { error: 'Session validation failed' },
      { status: 503 },
    );
  }
}
