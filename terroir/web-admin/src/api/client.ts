// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Fetch wrapper pour terroir-core via ARMAGEDDON :8080/api/terroir/core/*.
// Inclut JWT (cookie ory_kratos_session) + W3C traceparent pour Jaeger.

import type {
  Producer,
  ProducerListResponse,
  ProducerListQuery,
  Parcel,
  ParcelListResponse,
  ParcelListQuery,
  EudrValidation,
  Dds,
  AuditListResponse,
  AuditQuery,
  DashboardKpis,
  Cooperative,
  Uuid,
} from './types';

const API_BASE = '/api/terroir/core';

/**
 * Generate a W3C traceparent compatible header value.
 * Format: 00-<32 hex>-<16 hex>-01
 */
function genTraceparent(): string {
  const hex = (n: number) =>
    Array.from(crypto.getRandomValues(new Uint8Array(n)))
      .map((b) => b.toString(16).padStart(2, '0'))
      .join('');
  return `00-${hex(16)}-${hex(8)}-01`;
}

export class ApiError extends Error {
  status: number;
  body?: string;
  constructor(status: number, message: string, body?: string) {
    super(message);
    this.status = status;
    this.body = body;
  }
}

async function apiFetch<T>(
  path: string,
  init: RequestInit = {},
): Promise<T> {
  const headers = new Headers(init.headers);
  headers.set('Accept', 'application/json');
  if (init.body && !headers.has('Content-Type')) {
    headers.set('Content-Type', 'application/json');
  }
  headers.set('traceparent', genTraceparent());

  const res = await fetch(`${API_BASE}${path}`, {
    ...init,
    headers,
    credentials: 'include',
  });

  if (res.status === 401) {
    // Session expirée → redirect login (le composant RequireAuth re-route).
    window.location.assign('/login');
    throw new ApiError(401, 'Session expired');
  }
  if (!res.ok) {
    const txt = await res.text();
    throw new ApiError(res.status, `API ${res.status} on ${path}`, txt);
  }
  if (res.status === 204) {
    return undefined as T;
  }
  return (await res.json()) as T;
}

function buildQuery(params: Record<string, unknown>): string {
  const usp = new URLSearchParams();
  for (const [k, v] of Object.entries(params)) {
    if (v === undefined || v === null) continue;
    if (Array.isArray(v)) {
      usp.append(k, v.join(','));
    } else {
      usp.append(k, String(v));
    }
  }
  const s = usp.toString();
  return s ? `?${s}` : '';
}

// ----- Dashboard -----

export const fetchDashboardKpis = (): Promise<DashboardKpis> =>
  apiFetch('/dashboard/kpis');

// ----- Producers -----

export const listProducers = (
  q: ProducerListQuery = {},
): Promise<ProducerListResponse> =>
  apiFetch(`/producers${buildQuery(q as Record<string, unknown>)}`);

export const getProducer = (id: Uuid): Promise<Producer> =>
  apiFetch(`/producers/${id}`);

export const approveKyc = (id: Uuid): Promise<Producer> =>
  apiFetch(`/producers/${id}/kyc/approve`, { method: 'POST' });

export const suspendProducer = (id: Uuid, reason: string): Promise<Producer> =>
  apiFetch(`/producers/${id}/suspend`, {
    method: 'POST',
    body: JSON.stringify({ reason }),
  });

export const resetMfa = (id: Uuid): Promise<Producer> =>
  apiFetch(`/producers/${id}/mfa/reset`, { method: 'POST' });

// ----- Parcels -----

export const listParcels = (
  q: ParcelListQuery = {},
): Promise<ParcelListResponse> =>
  apiFetch(`/parcels${buildQuery(q as Record<string, unknown>)}`);

export const getParcel = (id: Uuid): Promise<Parcel> =>
  apiFetch(`/parcels/${id}`);

export const getParcelsByProducer = (producerId: Uuid): Promise<Parcel[]> =>
  apiFetch(`/producers/${producerId}/parcels`);

export const getEudrValidation = (parcelId: Uuid): Promise<EudrValidation> =>
  apiFetch(`/parcels/${parcelId}/eudr`);

// ----- DDS -----

export const getDdsForParcel = (parcelId: Uuid): Promise<Dds | null> =>
  apiFetch(`/parcels/${parcelId}/dds`);

export const submitDdsToTracesNt = (ddsId: Uuid): Promise<Dds> =>
  apiFetch(`/dds/${ddsId}/submit`, { method: 'POST' });

// ----- Audit -----

export const listAudit = (q: AuditQuery = {}): Promise<AuditListResponse> =>
  apiFetch(`/audit${buildQuery(q as Record<string, unknown>)}`);

// ----- Cooperatives -----

export const listCooperatives = (): Promise<Cooperative[]> =>
  apiFetch('/cooperatives');
