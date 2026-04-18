// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

/**
 * Client HTTP vers les endpoints admin Temporal exposés par poulets-api.
 *
 * En prod, poulets-api (Spring Boot) expose `/api/admin/workflows/*`
 * et utilise le Temporal Java SDK pour interroger le cluster Temporal.
 * Le BFF Next.js ne parle JAMAIS directement à Temporal (gRPC) — il passe
 * toujours par poulets-api qui gère l'authn/authz admin + auditing.
 */

const POULETS_API_URL = process.env.POULETS_API_URL ?? 'http://localhost:8901';

export interface WorkflowListFilters {
  type?: string;
  status?: string;
  actorId?: string;
}

async function apiFetch(path: string, init: RequestInit = {}, jwt?: string): Promise<Response> {
  const headers = new Headers(init.headers);
  if (jwt) headers.set('Authorization', `Bearer ${jwt}`);
  if (!headers.has('Content-Type') && init.body) {
    headers.set('Content-Type', 'application/json');
  }
  return fetch(`${POULETS_API_URL}${path}`, { ...init, headers });
}

export const temporalClient = {
  list(filters: WorkflowListFilters, jwt?: string): Promise<Response> {
    const params = new URLSearchParams();
    if (filters.type) params.set('type', filters.type);
    if (filters.status) params.set('status', filters.status);
    if (filters.actorId) params.set('actorId', filters.actorId);
    const qs = params.toString();
    return apiFetch(`/api/admin/workflows${qs ? '?' + qs : ''}`, {}, jwt);
  },
  get(id: string, jwt?: string): Promise<Response> {
    return apiFetch(`/api/admin/workflows/${encodeURIComponent(id)}`, {}, jwt);
  },
  history(id: string, jwt?: string): Promise<Response> {
    return apiFetch(`/api/admin/workflows/${encodeURIComponent(id)}/history`, {}, jwt);
  },
  activities(id: string, jwt?: string): Promise<Response> {
    return apiFetch(`/api/admin/workflows/${encodeURIComponent(id)}/activities`, {}, jwt);
  },
  signal(id: string, name: string, payload: unknown, jwt?: string): Promise<Response> {
    return apiFetch(
      `/api/admin/workflows/${encodeURIComponent(id)}/signal`,
      { method: 'POST', body: JSON.stringify({ name, payload }) },
      jwt,
    );
  },
  cancel(id: string, reason: string, jwt?: string): Promise<Response> {
    return apiFetch(
      `/api/admin/workflows/${encodeURIComponent(id)}/cancel`,
      { method: 'POST', body: JSON.stringify({ reason }) },
      jwt,
    );
  },
  terminate(id: string, reason: string, jwt?: string): Promise<Response> {
    return apiFetch(
      `/api/admin/workflows/${encodeURIComponent(id)}/terminate`,
      { method: 'POST', body: JSON.stringify({ reason }) },
      jwt,
    );
  },
  latencyStats(jwt?: string): Promise<Response> {
    return apiFetch('/api/admin/workflows/stats/latency', {}, jwt);
  },
};
