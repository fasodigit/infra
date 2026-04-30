// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * Helper proxy générique pour les routes BFF admin.
 *
 * Prend un `AdminAuthOk` (déjà validé) et un descriptif de la requête
 * upstream (méthode, path relatif sur ARMAGEDDON, body éventuel, query
 * params) et renvoie une `NextResponse` reconstruite avec :
 *   - le code de statut upstream
 *   - le body JSON (ou texte / blob selon le content-type)
 *   - les headers traceparent + X-Trace-Id
 *
 * On utilise `cache: 'no-store'` pour toutes les routes admin (le brief
 * exige une fraîcheur stricte). Les exceptions (cache 30 s sur GET
 * settings) sont gérées par les handlers eux-mêmes via les headers de
 * réponse Next, pas via le fetch upstream.
 *
 * Phase 4.b.7 — interceptor step-up : si l'upstream auth-ms répond
 * `401 + body { error: "step_up_required", methods_available, step_up_session_id, expires_at }`,
 * le proxy le **transmet tel quel** au frontend (status + body intacts) —
 * c'est le frontend (`StepUpInterceptor`) qui ouvre le modal `<faso-step-up-guard>`
 * et rejoue la requête originale avec le `stepUpToken` reçu.
 */

import { NextResponse } from 'next/server';

import type { AdminAuthOk } from '@/lib/admin-auth';
import { upstreamHeaders } from '@/lib/admin-auth';

const ARMAGEDDON_URL = process.env.ARMAGEDDON_URL ?? 'http://localhost:8080';
const AUTH_MS_URL = process.env.AUTH_MS_URL ?? 'http://localhost:8801';

export type UpstreamTarget = 'armageddon' | 'auth-ms';

export interface ProxyOptions {
  method: 'GET' | 'POST' | 'PUT' | 'DELETE' | 'PATCH';
  /** Chemin relatif à la racine du service cible. Doit commencer par `/`. */
  path: string;
  target?: UpstreamTarget;
  body?: unknown;
  query?: URLSearchParams | Record<string, string | number | boolean | undefined>;
  idempotencyKey?: string;
  extraHeaders?: Record<string, string>;
  /** Forwarder le body brut sans parsing (utilisé pour CSV export). */
  rawResponse?: boolean;
}

function baseUrl(target: UpstreamTarget): string {
  return target === 'auth-ms' ? AUTH_MS_URL : ARMAGEDDON_URL;
}

function buildUrl(target: UpstreamTarget, path: string, query: ProxyOptions['query']): string {
  const url = new URL(path, baseUrl(target));
  if (query) {
    if (query instanceof URLSearchParams) {
      query.forEach((v, k) => url.searchParams.append(k, v));
    } else {
      for (const [k, v] of Object.entries(query)) {
        if (v === undefined || v === null) continue;
        url.searchParams.set(k, String(v));
      }
    }
  }
  return url.toString();
}

/**
 * Effectue l'appel upstream et renvoie la `NextResponse` finale.
 */
export async function adminProxy(
  auth: AdminAuthOk,
  opts: ProxyOptions,
): Promise<NextResponse> {
  const target: UpstreamTarget = opts.target ?? 'armageddon';
  const url = buildUrl(target, opts.path, opts.query);
  const hasJsonBody = opts.body !== undefined && opts.method !== 'GET';
  const headers = upstreamHeaders(auth, {
    contentType: hasJsonBody ? 'application/json' : undefined,
    idempotencyKey: opts.idempotencyKey,
    extra: opts.extraHeaders,
  });

  let upstream: Response;
  try {
    upstream = await fetch(url, {
      method: opts.method,
      headers,
      body: hasJsonBody ? JSON.stringify(opts.body) : undefined,
      cache: 'no-store',
    });
  } catch (err) {
    console.error('[admin-proxy] upstream fetch failed', url, err);
    return NextResponse.json(
      { error: 'upstream_unavailable' },
      { status: 502, headers: { 'X-Trace-Id': auth.traceId } },
    );
  }

  const respHeaders: Record<string, string> = {
    'X-Trace-Id': auth.traceId,
  };

  if (opts.rawResponse) {
    const buf = await upstream.arrayBuffer();
    const ct = upstream.headers.get('content-type') ?? 'application/octet-stream';
    respHeaders['Content-Type'] = ct;
    const cd = upstream.headers.get('content-disposition');
    if (cd) respHeaders['Content-Disposition'] = cd;
    return new NextResponse(buf, { status: upstream.status, headers: respHeaders });
  }

  const contentType = upstream.headers.get('content-type') ?? '';
  if (contentType.includes('application/json')) {
    const json = await upstream.json().catch(() => ({}));
    // Phase 4.b.7 — pass-through 401 step_up_required so the Angular
    // StepUpInterceptor can open the guard modal and retry the original
    // request with the freshly issued stepUpToken.
    if (
      upstream.status === 401 &&
      json &&
      typeof json === 'object' &&
      (json as { error?: unknown }).error === 'step_up_required'
    ) {
      respHeaders['X-Step-Up-Required'] = '1';
    }
    return NextResponse.json(json, { status: upstream.status, headers: respHeaders });
  }
  const text = await upstream.text();
  if (!text) {
    return new NextResponse(null, { status: upstream.status, headers: respHeaders });
  }
  respHeaders['Content-Type'] = contentType || 'text/plain';
  return new NextResponse(text, { status: upstream.status, headers: respHeaders });
}
