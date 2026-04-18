// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import crypto from 'node:crypto';

const IMPRESSION_URL = process.env.IMPRESSION_SERVICE_URL ?? 'http://localhost:8921';
const INTERNAL_AUTH_SECRET = process.env.INTERNAL_AUTH_SECRET ?? 'dev-renderer-secret';

/**
 * Génère un header HMAC-SHA256 pour l'auth interne cross-service.
 * Format : X-Internal-Auth: {timestamp}:{hmac_hex}
 * Canonical string : `{timestamp}:{method}:{path}`
 */
function hmacHeader(method: string, path: string): string {
  const timestamp = Math.floor(Date.now() / 1000).toString();
  const canonical = `${timestamp}:${method.toUpperCase()}:${path}`;
  const hmac = crypto
    .createHmac('sha256', INTERNAL_AUTH_SECRET)
    .update(canonical)
    .digest('hex');
  return `${timestamp}:${hmac}`;
}

export interface ImpressionGenerateInput {
  type: 'CERTIFICAT_HALAL' | 'CONTRAT_COMMANDE' | 'RECEPISSE_LIVRAISON' | 'ATTESTATION_ELEVAGE';
  documentId: string;
  data: Record<string, unknown>;
}

export async function impressionFetch(
  path: string,
  init: RequestInit = {},
): Promise<Response> {
  const method = (init.method ?? 'GET').toUpperCase();
  const headers = new Headers(init.headers);
  headers.set('X-Internal-Auth', hmacHeader(method, path));
  if (!headers.has('Content-Type') && init.body) {
    headers.set('Content-Type', 'application/json');
  }
  return fetch(`${IMPRESSION_URL}${path}`, { ...init, headers });
}

export const impressionClient = {
  generate(input: ImpressionGenerateInput, jwt?: string): Promise<Response> {
    const headers = new Headers();
    if (jwt) headers.set('Authorization', `Bearer ${jwt}`);
    return impressionFetch('/api/impression/generate', {
      method: 'POST',
      body: JSON.stringify(input),
      headers,
    });
  },
  listQueue(jwt?: string): Promise<Response> {
    const headers = new Headers();
    if (jwt) headers.set('Authorization', `Bearer ${jwt}`);
    return impressionFetch('/api/impression/queue', { headers });
  },
  getJob(id: string, jwt?: string): Promise<Response> {
    const headers = new Headers();
    if (jwt) headers.set('Authorization', `Bearer ${jwt}`);
    return impressionFetch(`/api/impression/${encodeURIComponent(id)}`, { headers });
  },
  downloadPdf(id: string, jwt?: string): Promise<Response> {
    const headers = new Headers();
    if (jwt) headers.set('Authorization', `Bearer ${jwt}`);
    return impressionFetch(`/api/impression/${encodeURIComponent(id)}/pdf`, { headers });
  },
  verifyQr(code: string, jwt?: string): Promise<Response> {
    const headers = new Headers();
    if (jwt) headers.set('Authorization', `Bearer ${jwt}`);
    return impressionFetch(`/api/verification/qr/${encodeURIComponent(code)}`, {
      method: 'POST',
      headers,
    });
  },
};
