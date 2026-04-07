import { Configuration, FrontendApi, IdentityApi } from '@ory/client';

const kratosPublicUrl = process.env.KRATOS_PUBLIC_URL || 'http://localhost:4433';
const kratosAdminUrl = process.env.KRATOS_ADMIN_URL || 'http://localhost:4434';

/**
 * Kratos Frontend API client (public endpoints).
 * Used for login flows, registration flows, session checks, etc.
 */
export const kratosFrontend = new FrontendApi(
  new Configuration({
    basePath: kratosPublicUrl,
    baseOptions: {
      // Forward cookies from the original request
      withCredentials: true,
    },
  }),
);

/**
 * Kratos Identity API client (admin endpoints).
 * Used for admin operations: identity management, session invalidation.
 */
export const kratosAdmin = new IdentityApi(
  new Configuration({
    basePath: kratosAdminUrl,
  }),
);

/**
 * Extract the cookie header from an incoming request to forward to Kratos.
 */
export function extractCookies(headers: Headers): string {
  return headers.get('cookie') || '';
}

/**
 * Map a Kratos identity/session to our simplified UserSession type.
 */
export interface UserSession {
  id: string;
  email: string;
  name: string;
  role: 'client' | 'eleveur' | 'admin';
  verified: boolean;
}

export function mapKratosSession(session: any): UserSession {
  const identity = session.identity;
  const traits = identity?.traits || {};

  return {
    id: identity?.id || '',
    email: traits.email || '',
    name: traits.name || traits.email || '',
    role: traits.role || 'client',
    verified: identity?.verifiable_addresses?.[0]?.verified || false,
  };
}
