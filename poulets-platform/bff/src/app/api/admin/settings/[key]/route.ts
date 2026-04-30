// SPDX-License-Identifier: AGPL-3.0-or-later
// (c) 2026 FASO DIGITALISATION - Burkina Faso

/**
 * GET /api/admin/settings/:key — détail d'un setting.
 * PUT /api/admin/settings/:key — mise à jour CAS sur version (409 si stale).
 *
 * GET : MANAGER. PUT : SUPER-ADMIN (le brief précise édition SUPER-ADMIN
 * stricte ; auth-ms revérifie via Keto `update_settings`).
 *
 * Cache GET : 30 s avec ETag = "v{version}".
 */

import { NextResponse, type NextRequest } from 'next/server';

import { adminAuth, upstreamHeaders } from '@/lib/admin-auth';
import { auditLog } from '@/lib/admin-audit';
import { adminProxy } from '@/lib/admin-proxy';
import { extractIdempotencyKey } from '@/lib/admin-otp';
import { SettingUpdateSchema } from '@/lib/schemas/admin';

const ARMAGEDDON_URL = process.env.ARMAGEDDON_URL ?? 'http://localhost:8080';

export async function GET(
  req: NextRequest,
  { params }: { params: Promise<{ key: string }> },
) {
  const auth = await adminAuth(req, { requiredLevel: 'MANAGER' });
  if (!auth.ok) return auth.response;
  const { key } = await params;

  const upstream = await fetch(
    `${ARMAGEDDON_URL}/admin/settings/${encodeURIComponent(key)}`,
    {
      method: 'GET',
      headers: upstreamHeaders(auth),
      cache: 'no-store',
    },
  ).catch(() => null);

  if (!upstream) {
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
  const payload = (await upstream.json()) as { version?: number };
  const etag = `"setting-${key}-v${payload?.version ?? 0}"`;
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

export async function PUT(
  req: NextRequest,
  { params }: { params: Promise<{ key: string }> },
) {
  const auth = await adminAuth(req, { requiredLevel: 'SUPER-ADMIN' });
  if (!auth.ok) return auth.response;
  const { key } = await params;
  const parsed = SettingUpdateSchema.safeParse(await req.json().catch(() => ({})));
  if (!parsed.success) {
    return NextResponse.json({ error: parsed.error.format() }, { status: 400 });
  }
  const idempotencyKey = extractIdempotencyKey(req);
  const res = await adminProxy(auth, {
    method: 'PUT',
    path: `/admin/settings/${encodeURIComponent(key)}`,
    body: parsed.data,
    idempotencyKey,
  });
  if (res.status >= 200 && res.status < 300) {
    void auditLog(
      {
        action: 'SETTINGS_UPDATED',
        actor: { userId: auth.userId, email: auth.email, role: auth.rawRole, ip: auth.ip },
        target: { type: 'setting', id: key },
        metadata: { version: parsed.data.version, motif: parsed.data.motif },
        newValue: parsed.data.value,
        traceId: auth.traceparent,
        critical: true,
      },
      { authToken: auth.jwt, idempotencyKey },
    );
  }
  return res;
}
