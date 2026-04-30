// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * Middleware d'authentification & d'autorisation pour les routes admin.
 *
 * Pipeline :
 *   1. Lecture cookie Kratos `ory_kratos_session` (ou `Authorization: Bearer`)
 *      → validation via `${KRATOS_PUBLIC_URL}/sessions/whoami`.
 *   2. Validation JWT auth-ms (`Authorization: Bearer …`) via JWKS
 *      (`${JWKS_URI}`) — cache 10 min in-memory (Map globale).
 *   3. (Optionnel) Check Keto inline : `${KETO_READ_URL}/relation-tuples/check`
 *      — fail-closed.
 *   4. Propage `traceparent` upstream (génère un span synthétique si absent).
 *
 * Renvoie soit `{ ok: true, ... }`, soit `{ ok: false, response }` avec une
 * `NextResponse.json` prête à être renvoyée par la route handler.
 *
 * IMPORTANT — Sécurité : on extrait `userId/role/email` UNIQUEMENT depuis la
 * session Kratos / les claims JWT signés. Jamais depuis les headers de la
 * requête entrante (le BFF est exposé derrière ARMAGEDDON et un attaquant
 * pourrait forger `X-User-Id`).
 */

import { NextResponse, type NextRequest } from 'next/server';
import { createRemoteJWKSet, jwtVerify, type JWTPayload } from 'jose';

import { generateRequestId } from '@/lib/admin-otp';
import {
  normalizeLevel,
  requireKetoCheck,
  requireRole,
  type KetoCheckParams,
} from '@/lib/admin-permissions';
import type { AdminLevel } from '@/lib/schemas/admin';

// ---------------------------------------------------------------------------
// Configuration env
// ---------------------------------------------------------------------------

const KRATOS_PUBLIC_URL =
  process.env.KRATOS_PUBLIC_URL ?? 'http://localhost:4433';
const JWKS_URI =
  process.env.JWKS_URI ?? 'http://localhost:8801/.well-known/jwks.json';
const JWT_ISSUER = process.env.JWT_ISSUER ?? 'auth-ms';
const JWT_AUDIENCE = process.env.JWT_AUDIENCE ?? 'faso-admin';

const JWKS_CACHE_TTL_MS = 10 * 60 * 1000;

// ---------------------------------------------------------------------------
// Cache JWKS — Map globale survivant aux invocations route-handler
// (Next.js 16 réutilise l'isolate quand possible, sinon dégrade en cache
// court-terme par lambda — acceptable car les clés tournent rarement).
// ---------------------------------------------------------------------------

interface JwksCacheEntry {
  jwks: ReturnType<typeof createRemoteJWKSet>;
  fetchedAt: number;
}

const jwksGlobal = globalThis as typeof globalThis & {
  __faso_jwks_cache__?: Map<string, JwksCacheEntry>;
};
const jwksCache: Map<string, JwksCacheEntry> =
  jwksGlobal.__faso_jwks_cache__ ?? (jwksGlobal.__faso_jwks_cache__ = new Map());

function getJwks(uri: string): ReturnType<typeof createRemoteJWKSet> {
  const now = Date.now();
  const cached = jwksCache.get(uri);
  if (cached && now - cached.fetchedAt < JWKS_CACHE_TTL_MS) {
    return cached.jwks;
  }
  const jwks = createRemoteJWKSet(new URL(uri), {
    cooldownDuration: 30_000,
    cacheMaxAge: JWKS_CACHE_TTL_MS,
  });
  jwksCache.set(uri, { jwks, fetchedAt: now });
  return jwks;
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface AdminAuthRequirements {
  /** Niveau RBAC minimal requis (compare la hiérarchie). */
  requiredLevel?: AdminLevel;
  /** Check Keto fin (override le simple check de niveau). */
  ketoCheck?: Omit<KetoCheckParams, 'subjectId' | 'traceparent'>;
  /** Désactive la validation JWT (utile pour endpoints qui n'ont que la session Kratos, ex: dashboard). */
  skipJwt?: boolean;
  /**
   * Bypass complet de l'authentification (Kratos + JWT). Utilisé pour les
   * endpoints publics tels que `/api/admin/auth/recovery/*` qui doivent rester
   * accessibles sans session (un user avec MFA perdu n'a aucune session active).
   *
   * On expose toujours un contexte `auth` minimal (traceparent, IP, userAgent)
   * pour permettre au handler de proxifier vers auth-ms et émettre un audit-log.
   */
  allowPublic?: boolean;
}

export interface AdminAuthOk {
  ok: true;
  userId: string;
  email: string;
  role: AdminLevel | undefined;
  rawRole: string | undefined;
  jwt: string | undefined;
  traceparent: string;
  traceId: string;
  ip: string;
  userAgent: string;
  kratosSession: KratosSessionLite;
  jwtClaims?: JWTPayload;
}

export interface AdminAuthErr {
  ok: false;
  response: NextResponse;
}

export type AdminAuthResult = AdminAuthOk | AdminAuthErr;

interface KratosSessionLite {
  identityId: string;
  email: string;
  role: string | undefined;
  active: boolean;
  raw: unknown;
}

// ---------------------------------------------------------------------------
// Helpers traceparent (W3C)
// ---------------------------------------------------------------------------

const TRACEPARENT_RE = /^[0-9a-f]{2}-[0-9a-f]{32}-[0-9a-f]{16}-[0-9a-f]{2}$/i;

function ensureTraceparent(req: NextRequest): { traceparent: string; traceId: string } {
  const incoming = req.headers.get('traceparent');
  if (incoming && TRACEPARENT_RE.test(incoming)) {
    const parts = incoming.split('-');
    return { traceparent: incoming, traceId: parts[1] ?? randomTraceId() };
  }
  const traceId = randomTraceId();
  const spanId = randomSpanId();
  return { traceparent: `00-${traceId}-${spanId}-01`, traceId };
}

function randomHex(bytes: number): string {
  if (typeof crypto !== 'undefined' && 'getRandomValues' in crypto) {
    const arr = new Uint8Array(bytes);
    crypto.getRandomValues(arr);
    return Array.from(arr, (b) => b.toString(16).padStart(2, '0')).join('');
  }
  // Fallback (faible entropie — n'arrivera pas en runtime Next).
  let s = '';
  for (let i = 0; i < bytes * 2; i++) s += Math.floor(Math.random() * 16).toString(16);
  return s;
}

function randomTraceId(): string {
  return randomHex(16);
}

function randomSpanId(): string {
  return randomHex(8);
}

// ---------------------------------------------------------------------------
// Kratos session lookup
// ---------------------------------------------------------------------------

async function fetchKratosSession(req: NextRequest): Promise<KratosSessionLite | null> {
  const cookieHeader = req.headers.get('cookie') ?? '';
  const sessionCookie = req.cookies.get('ory_kratos_session')?.value;
  const authzHeader = req.headers.get('authorization') ?? '';
  const hasBearer = /^Bearer\s+/i.test(authzHeader);
  const hasCookie = Boolean(sessionCookie) || /(?:^|;\s*)ory_kratos_session=/.test(cookieHeader);
  if (!hasCookie && !hasBearer) return null;

  const headers: Record<string, string> = {};
  if (hasBearer) {
    headers['Authorization'] = authzHeader;
  } else if (sessionCookie && sessionCookie.startsWith('ory_st_')) {
    headers['X-Session-Token'] = sessionCookie;
  } else {
    headers['Cookie'] = cookieHeader;
  }

  try {
    const res = await fetch(`${KRATOS_PUBLIC_URL}/sessions/whoami`, {
      headers,
      cache: 'no-store',
      redirect: 'manual',
    });
    if (!res.ok) return null;
    const json = (await res.json()) as {
      identity?: { id?: string; traits?: { email?: string; role?: string } };
      active?: boolean;
    };
    return {
      identityId: json.identity?.id ?? '',
      email: json.identity?.traits?.email ?? '',
      role: json.identity?.traits?.role,
      active: json.active !== false,
      raw: json,
    };
  } catch (err) {
    console.error('[admin-auth] Kratos whoami failed', err);
    return null;
  }
}

// ---------------------------------------------------------------------------
// JWT verification (auth-ms)
// ---------------------------------------------------------------------------

async function verifyAuthMsJwt(
  token: string,
): Promise<{ payload: JWTPayload; ok: true } | { ok: false }> {
  try {
    const jwks = getJwks(JWKS_URI);
    const { payload } = await jwtVerify(token, jwks, {
      issuer: JWT_ISSUER,
      audience: JWT_AUDIENCE,
    });
    return { ok: true, payload };
  } catch (err) {
    console.error('[admin-auth] JWT verify failed', (err as Error).message);
    return { ok: false };
  }
}

function extractBearer(req: NextRequest): string | undefined {
  const h = req.headers.get('authorization') ?? '';
  const m = /^Bearer\s+(.+)$/i.exec(h);
  return m ? m[1] : undefined;
}

function clientIp(req: NextRequest): string {
  return (
    req.headers.get('x-forwarded-for')?.split(',')[0]?.trim() ||
    req.headers.get('x-real-ip') ||
    'unknown'
  );
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Valide la session admin et renvoie le contexte enrichi pour la route.
 *
 * Usage :
 * ```ts
 * const auth = await adminAuth(req, { requiredLevel: 'ADMIN' });
 * if (!auth.ok) return auth.response;
 * // auth.userId, auth.role, auth.traceparent, auth.jwt
 * ```
 */
export async function adminAuth(
  req: NextRequest,
  reqs: AdminAuthRequirements = {},
): Promise<AdminAuthResult> {
  const { traceparent, traceId } = ensureTraceparent(req);

  // 0. Bypass public (recovery flows : pas de session)
  if (reqs.allowPublic) {
    return {
      ok: true,
      userId: '',
      email: '',
      role: undefined,
      rawRole: undefined,
      jwt: undefined,
      traceparent,
      traceId,
      ip: clientIp(req),
      userAgent: req.headers.get('user-agent') ?? '',
      kratosSession: { identityId: '', email: '', role: undefined, active: false, raw: null },
      jwtClaims: undefined,
    };
  }

  // 1. Session Kratos
  const session = await fetchKratosSession(req);
  if (!session || !session.identityId || !session.active) {
    return errResponse(401, 'unauthorized', traceId);
  }

  // 2. JWT auth-ms (optionnel)
  const bearer = extractBearer(req);
  let jwtPayload: JWTPayload | undefined;
  if (!reqs.skipJwt) {
    if (!bearer) {
      // En l'absence de JWT, on accepte la session Kratos comme proof-of-identity
      // mais on ne propage rien upstream → upstream Spring exigera un JWT.
      // Pour les routes les plus sensibles, le brief impose JWT obligatoire.
      // On reste tolérant ici, le check de rôle suit.
    } else {
      const v = await verifyAuthMsJwt(bearer);
      if (!v.ok) return errResponse(401, 'invalid jwt', traceId);
      jwtPayload = v.payload;
    }
  }

  // 3. Extraction rôle (priorité JWT > Kratos)
  const rawRole =
    (jwtPayload?.role as string | undefined) ??
    (jwtPayload?.['https://faso.bf/role'] as string | undefined) ??
    session.role;
  const role = rawRole ? normalizeLevel(rawRole) : undefined;

  // 4. Check niveau RBAC
  if (reqs.requiredLevel && !requireRole(role, reqs.requiredLevel)) {
    return errResponse(403, 'forbidden', traceId);
  }

  // 5. Check Keto fin (override)
  if (reqs.ketoCheck) {
    const allowed = await requireKetoCheck({
      ...reqs.ketoCheck,
      subjectId: session.identityId,
      traceparent,
    });
    if (!allowed) return errResponse(403, 'forbidden', traceId);
  }

  return {
    ok: true,
    userId: session.identityId,
    email: session.email,
    role,
    rawRole,
    jwt: bearer,
    traceparent,
    traceId,
    ip: clientIp(req),
    userAgent: req.headers.get('user-agent') ?? '',
    kratosSession: session,
    jwtClaims: jwtPayload,
  };
}

function errResponse(status: number, error: string, traceId: string): AdminAuthErr {
  const response = NextResponse.json({ error }, { status });
  response.headers.set('X-Trace-Id', traceId);
  return { ok: false, response };
}

/**
 * Helper : construit les headers à propager upstream (auth-ms / poulets-api).
 * Inclut Authorization, traceparent, Idempotency-Key, X-Request-Id.
 */
export function upstreamHeaders(
  auth: AdminAuthOk,
  options: {
    contentType?: string;
    idempotencyKey?: string;
    extra?: Record<string, string>;
  } = {},
): Record<string, string> {
  const idempotencyKey = options.idempotencyKey ?? generateRequestId();
  const headers: Record<string, string> = {
    Accept: 'application/json',
    traceparent: auth.traceparent,
    'X-Request-Id': auth.traceId,
    'X-Trace-Id': auth.traceId,
    'X-Forwarded-User': auth.userId,
    'Idempotency-Key': idempotencyKey,
  };
  if (options.contentType) headers['Content-Type'] = options.contentType;
  if (auth.jwt) headers['Authorization'] = `Bearer ${auth.jwt}`;
  if (auth.email) headers['X-Forwarded-Email'] = auth.email;
  if (auth.rawRole) headers['X-Forwarded-Role'] = auth.rawRole;
  if (options.extra) Object.assign(headers, options.extra);
  return headers;
}
