// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * GET /api/admin/settings — bundle complet des paramètres groupés par catégorie.
 *
 * Cache : 30 s (s-maxage), ETag basé sur MAX(version) renvoyé par auth-ms.
 * Niveau requis : MANAGER (lecture).
 */

import { NextResponse, type NextRequest } from 'next/server';

import { adminAuth, upstreamHeaders } from '@/lib/admin-auth';

const ARMAGEDDON_URL = process.env.ARMAGEDDON_URL ?? 'http://localhost:8080';

interface SettingDto {
  version?: number;
  [k: string]: unknown;
}

function computeMaxVersion(payload: unknown): number {
  let max = 0;
  function walk(node: unknown): void {
    if (!node) return;
    if (Array.isArray(node)) {
      for (const item of node) walk(item);
      return;
    }
    if (typeof node === 'object') {
      const obj = node as SettingDto;
      if (typeof obj.version === 'number' && obj.version > max) max = obj.version;
      for (const v of Object.values(obj)) walk(v);
    }
  }
  walk(payload);
  return max;
}

export async function GET(req: NextRequest) {
  const auth = await adminAuth(req, { requiredLevel: 'MANAGER' });
  if (!auth.ok) return auth.response;

  const headers = upstreamHeaders(auth);
  let upstream: Response;
  try {
    upstream = await fetch(`${ARMAGEDDON_URL}/admin/settings`, {
      method: 'GET',
      headers,
      cache: 'no-store',
    });
  } catch (err) {
    console.error('[admin/settings] upstream failed', err);
    return NextResponse.json(
      { error: 'upstream_unavailable' },
      { status: 502, headers: { 'X-Trace-Id': auth.traceId } },
    );
  }

  if (!upstream.ok) {
    const payload = await upstream.json().catch(() => ({}));
    return NextResponse.json(payload, {
      status: upstream.status,
      headers: { 'X-Trace-Id': auth.traceId },
    });
  }

  const payload = await upstream.json();
  const maxVersion = computeMaxVersion(payload);
  const etag = `"settings-v${maxVersion}"`;

  // Conditional GET — 304 si l'ETag correspond.
  const ifNoneMatch = req.headers.get('if-none-match');
  if (ifNoneMatch && ifNoneMatch === etag) {
    return new NextResponse(null, {
      status: 304,
      headers: {
        ETag: etag,
        'Cache-Control': 'private, max-age=30, stale-while-revalidate=10',
        'X-Trace-Id': auth.traceId,
      },
    });
  }

  return NextResponse.json(payload, {
    status: 200,
    headers: {
      ETag: etag,
      'Cache-Control': 'private, max-age=30, stale-while-revalidate=10',
      'X-Trace-Id': auth.traceId,
    },
  });
}
