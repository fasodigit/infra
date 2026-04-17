import { NextRequest, NextResponse } from 'next/server';

const kratosPublicUrl = process.env.KRATOS_PUBLIC_URL || 'http://localhost:4433';
const frontendUrl = process.env.FRONTEND_URL || 'http://localhost:4801';

// =============================================================================
// In-memory token bucket rate limiter for auth endpoints
// 10 requests / IP / 60-second window
// State is per-isolate; for multi-instance deployments use KAYA (redis://kaya:6379)
// =============================================================================
const RATE_LIMIT_MAX = 10;
const RATE_LIMIT_WINDOW_MS = 60_000;

interface Bucket {
  count: number;
  resetAt: number;
}

const rateLimitStore = new Map<string, Bucket>();

function checkRateLimit(ip: string): { allowed: boolean; retryAfter: number } {
  const now = Date.now();
  let bucket = rateLimitStore.get(ip);

  if (!bucket || now >= bucket.resetAt) {
    bucket = { count: 1, resetAt: now + RATE_LIMIT_WINDOW_MS };
    rateLimitStore.set(ip, bucket);
    return { allowed: true, retryAfter: 0 };
  }

  if (bucket.count >= RATE_LIMIT_MAX) {
    const retryAfter = Math.ceil((bucket.resetAt - now) / 1000);
    return { allowed: false, retryAfter };
  }

  bucket.count++;
  return { allowed: true, retryAfter: 0 };
}

function clientIp(request: NextRequest): string {
  return (
    request.headers.get('x-forwarded-for')?.split(',')[0]?.trim() ||
    request.headers.get('x-real-ip') ||
    'unknown'
  );
}

/**
 * Middleware that validates Kratos sessions for protected BFF routes.
 *
 * Rate-limited routes (/api/auth/login, /api/auth/register): 10 req/IP/min.
 * Other public routes (/api/auth/session, /api/auth/logout, /api/health) bypass auth.
 * All other /api/* routes (including /api/graphql) require a valid Kratos session.
 * Identity headers X-User-Id, X-User-Email, X-User-Role are injected before proxying.
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

  // Rate-limited auth mutation endpoints
  const rateLimitedPaths = ['/api/auth/login', '/api/auth/register'];
  if (rateLimitedPaths.some((p) => pathname.startsWith(p))) {
    const ip = clientIp(request);
    const { allowed, retryAfter } = checkRateLimit(ip);
    if (!allowed) {
      return addCorsHeaders(
        NextResponse.json(
          { error: 'Too Many Requests' },
          {
            status: 429,
            headers: {
              'Retry-After': String(retryAfter),
              'X-RateLimit-Limit': String(RATE_LIMIT_MAX),
              'X-RateLimit-Window': '60',
            },
          },
        ),
      );
    }
    return addCorsHeaders(NextResponse.next());
  }

  // Other public routes that don't require authentication
  const publicPaths = [
    '/api/auth/session',
    '/api/auth/logout',
    '/api/health',
  ];

  if (publicPaths.some((p) => pathname.startsWith(p))) {
    return addCorsHeaders(NextResponse.next());
  }

  // All other /api/* routes (including /api/graphql): require valid Kratos session
  // Identity headers are injected here so downstream services never trust client input
  if (pathname.startsWith('/api/')) {
    const cookie = request.headers.get('cookie') || '';
    const sessionToken = request.cookies.get('ory_kratos_session')?.value;

    if (!cookie && !sessionToken) {
      return addCorsHeaders(
        NextResponse.json({ error: 'Unauthorized' }, { status: 401 }),
      );
    }

    try {
      const kratosHeaders: Record<string, string> = {};
      if (sessionToken) {
        kratosHeaders['X-Session-Token'] = sessionToken;
      }
      if (cookie) {
        kratosHeaders['Cookie'] = cookie;
      }

      const sessionResponse = await fetch(
        `${kratosPublicUrl}/sessions/whoami`,
        {
          headers: kratosHeaders,
          redirect: 'manual',
        },
      );

      if (!sessionResponse.ok) {
        return addCorsHeaders(
          NextResponse.json({ error: 'Session expired' }, { status: 401 }),
        );
      }

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
