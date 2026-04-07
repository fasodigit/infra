import { NextRequest, NextResponse } from 'next/server';

const kratosPublicUrl = process.env.KRATOS_PUBLIC_URL || 'http://localhost:4433';
const frontendUrl = process.env.FRONTEND_URL || 'http://localhost:4801';

/**
 * Middleware that validates Kratos sessions for protected BFF routes.
 *
 * Public routes (/api/auth/*, /api/health) bypass validation.
 * All other /api/* routes require a valid Kratos session cookie.
 */
export async function middleware(request: NextRequest) {
  const { pathname } = request.nextUrl;

  // CORS preflight handling for Angular frontend
  if (request.method === 'OPTIONS') {
    return new NextResponse(null, {
      status: 204,
      headers: corsHeaders(),
    });
  }

  // Public routes that don't require authentication
  const publicPaths = [
    '/api/auth/login',
    '/api/auth/register',
    '/api/auth/session',
    '/api/auth/logout',
    '/api/health',
  ];

  if (publicPaths.some((p) => pathname.startsWith(p))) {
    return addCorsHeaders(NextResponse.next());
  }

  // GraphQL endpoint: proxy directly (auth handled by poulets-api via session cookie)
  if (pathname === '/api/graphql') {
    return addCorsHeaders(NextResponse.next());
  }

  // For all other /api/* routes, validate Kratos session
  if (pathname.startsWith('/api/')) {
    const cookie = request.headers.get('cookie') || '';

    if (!cookie) {
      return addCorsHeaders(
        NextResponse.json({ error: 'Unauthorized' }, { status: 401 }),
      );
    }

    try {
      const sessionResponse = await fetch(
        `${kratosPublicUrl}/sessions/whoami`,
        {
          headers: { cookie },
          // Don't follow redirects
          redirect: 'manual',
        },
      );

      if (!sessionResponse.ok) {
        return addCorsHeaders(
          NextResponse.json({ error: 'Session expired' }, { status: 401 }),
        );
      }

      // Session is valid, attach identity ID as header for downstream services
      const session = await sessionResponse.json();
      const response = NextResponse.next();
      response.headers.set('X-User-Id', session.identity?.id || '');
      response.headers.set('X-User-Email', session.identity?.traits?.email || '');
      response.headers.set('X-User-Role', session.identity?.traits?.role || 'client');

      return addCorsHeaders(response);
    } catch (error) {
      console.error('[BFF Middleware] Kratos session check failed:', error);
      return addCorsHeaders(
        NextResponse.json({ error: 'Auth service unavailable' }, { status: 503 }),
      );
    }
  }

  return NextResponse.next();
}

export const config = {
  matcher: '/api/:path*',
};

function corsHeaders(): HeadersInit {
  return {
    'Access-Control-Allow-Origin': frontendUrl,
    'Access-Control-Allow-Methods': 'GET, POST, PUT, DELETE, OPTIONS',
    'Access-Control-Allow-Headers': 'Content-Type, Authorization',
    'Access-Control-Allow-Credentials': 'true',
    'Access-Control-Max-Age': '86400',
  };
}

function addCorsHeaders(response: NextResponse): NextResponse {
  const headers = corsHeaders();
  for (const [key, value] of Object.entries(headers)) {
    response.headers.set(key, value);
  }
  return response;
}
