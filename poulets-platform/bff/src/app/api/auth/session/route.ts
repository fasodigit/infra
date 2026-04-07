import { NextRequest, NextResponse } from 'next/server';
import { kratosFrontend, mapKratosSession, extractCookies } from '@/lib/kratos';

/**
 * GET /api/auth/session
 * Validates the current Kratos session and returns user data.
 * Called by the Angular frontend on bootstrap to check if user is logged in.
 */
export async function GET(request: NextRequest) {
  const cookie = extractCookies(request.headers);

  if (!cookie) {
    return NextResponse.json(
      { error: 'No session cookie present' },
      { status: 401 },
    );
  }

  try {
    const { data: session } = await kratosFrontend.toSession({ cookie });

    if (!session?.active) {
      return NextResponse.json(
        { error: 'Session is not active' },
        { status: 401 },
      );
    }

    return NextResponse.json(mapKratosSession(session));
  } catch (error: any) {
    const status = error?.response?.status;

    if (status === 401 || status === 403) {
      return NextResponse.json(
        { error: 'Session expired or invalid' },
        { status: 401 },
      );
    }

    console.error('[BFF] Session validation error:', error?.message);
    return NextResponse.json(
      { error: 'Session validation failed' },
      { status: 503 },
    );
  }
}
