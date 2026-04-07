import { NextRequest, NextResponse } from 'next/server';

const POULETS_API_GRAPHQL =
  process.env.POULETS_API_GRAPHQL || 'http://localhost:8901/graphql';

/**
 * POST /api/graphql
 * Proxies GraphQL requests to the poulets-api backend.
 * Forwards session cookies and adds user identity headers.
 *
 * Note: This route also handles the case where Next.js rewrites don't apply
 * (e.g., when request includes custom headers that need forwarding).
 */
export async function POST(request: NextRequest) {
  try {
    const body = await request.text();
    const cookie = request.headers.get('cookie') || '';

    // Forward the GraphQL request to the backend
    const backendResponse = await fetch(POULETS_API_GRAPHQL, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        // Forward session cookie for backend-side auth
        Cookie: cookie,
        // Forward identity headers set by middleware
        'X-User-Id': request.headers.get('X-User-Id') || '',
        'X-User-Email': request.headers.get('X-User-Email') || '',
        'X-User-Role': request.headers.get('X-User-Role') || '',
      },
      body,
    });

    const data = await backendResponse.json();

    return NextResponse.json(data, {
      status: backendResponse.status,
    });
  } catch (error: any) {
    console.error('[BFF] GraphQL proxy error:', error?.message);
    return NextResponse.json(
      {
        errors: [
          {
            message: 'Backend service unavailable',
            extensions: { code: 'SERVICE_UNAVAILABLE' },
          },
        ],
      },
      { status: 503 },
    );
  }
}
