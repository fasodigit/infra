// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * CoreClient — wrapper REST `terroir-core` (Module 1 + 2 producteurs / parcelles).
 *
 * Couvre les endpoints P1.1 documentés dans
 * `INFRA/terroir/docs/ULTRAPLAN-TERROIR-2026-04-30.md` §6 P1.1 :
 *   - POST /producers          (PII chiffrées via Vault Transit)
 *   - GET  /producers/:id
 *   - GET  /producers          (list keyset-paginated)
 *   - PATCH /producers/:id
 *   - DELETE /producers/:id
 *   - POST /parcels
 *   - GET  /parcels/:id
 *   - POST /parcels/:id/polygon (Yjs CRDT delta b64)
 *   - GET  /parcels/:id/polygon
 *   - POST /households
 *
 * Le routage passe **toujours** par ARMAGEDDON (`/api/terroir/core/*`).
 * Authentification : `X-Tenant-Slug` (M2M loopback test-only) OU
 * `Authorization: Bearer <jwt>` produit par Kratos. Cf. CLAUDE.md §11/§12.
 *
 * Pas de mocks : la classe parle au vrai service Rust live.
 */
import {
  request,
  type APIRequestContext,
  type APIResponse,
} from '@playwright/test';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ProducerCreateRequest {
  cooperativeId: string;
  externalId?: string;
  fullName: string;
  nin: string;
  phone: string;
  photoUrl?: string;
  gpsDomicileLat: number;
  gpsDomicileLon: number;
  householdId?: string;
  primaryCrop: string;
}

export interface ProducerResponse {
  id: string;
  cooperativeId: string;
  externalId?: string;
  fullName: string;
  nin: string;
  phone: string;
  photoUrl?: string;
  gpsDomicileLat: number;
  gpsDomicileLon: number;
  householdId?: string;
  primaryCrop?: string;
  registeredAt: string;
  updatedAt: string;
  lwwVersion: number;
}

export interface ParcelCreateRequest {
  producerId: string;
  cropType?: string;
  plantedAt?: string; // YYYY-MM-DD
  surfaceHectares?: number;
}

export interface ParcelResponse {
  id: string;
  producerId: string;
  cropType?: string;
  plantedAt?: string;
  surfaceHectares?: number;
  registeredAt: string;
  updatedAt: string;
  lwwVersion: number;
}

export interface PolygonUpdateRequest {
  yjsUpdate: string; // base64
  geojson: Record<string, unknown>;
}

export interface PolygonResponse {
  parcelId: string;
  yjsState: string;
  geojson: Record<string, unknown>;
  geomWkt?: string;
  yjsVersion: number;
  updatedAt: string;
}

export interface CoreError {
  error: string;
  message?: string;
  details?: unknown;
}

export interface CoreResponse<T> {
  status: number;
  body: T | CoreError;
  headers: Record<string, string>;
  durationMs: number;
}

export interface CoreClientOptions {
  /** Override base URL (default ARMAGEDDON :8080). */
  baseURL?: string;
  /** Tenant slug (sans préfixe `terroir_t_`). Ex `t_pilot`. */
  tenantSlug?: string;
  /** Optional bearer token (Kratos JWT) — overrides X-Tenant-Slug auth. */
  bearer?: string;
  /** User ID for tenant context (when using X-Tenant-Slug, default `anonymous`). */
  userId?: string;
}

/** Génère un `traceparent` W3C aléatoire (sample bit = 01). */
function newTraceparent(): string {
  const hex = (n: number): string =>
    Array.from({ length: n }, () =>
      Math.floor(Math.random() * 16).toString(16),
    ).join('');
  return `00-${hex(32)}-${hex(16)}-01`;
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

export class CoreClient {
  private readonly baseURL: string;
  private readonly tenantSlug: string;
  private readonly bearer?: string;

  constructor(opts: CoreClientOptions = {}) {
    this.baseURL =
      opts.baseURL ?? process.env.TERROIR_GATEWAY_URL ?? 'http://localhost:8080';
    this.tenantSlug = opts.tenantSlug ?? process.env.TERROIR_TENANT_SLUG ?? 't_pilot';
    this.bearer = opts.bearer;
  }

  private prefix(): string {
    return `${this.baseURL}/api/terroir/core`;
  }

  private async api(): Promise<APIRequestContext> {
    const headers: Record<string, string> = {
      'content-type': 'application/json',
      accept: 'application/json',
    };
    if (this.bearer) {
      headers.Authorization = `Bearer ${this.bearer}`;
    } else {
      // M2M path — service trusts X-Tenant-Slug only on loopback (CLAUDE.md §11).
      headers['X-Tenant-Slug'] = this.tenantSlug;
    }
    return request.newContext({ extraHTTPHeaders: headers });
  }

  private async wrap<T>(p: Promise<APIResponse>): Promise<CoreResponse<T>> {
    const start = Date.now();
    const res = await p;
    const durationMs = Date.now() - start;
    let body: T | CoreError;
    try {
      body = (await res.json()) as T | CoreError;
    } catch {
      body = { error: 'invalid_json' } as CoreError;
    }
    return {
      status: res.status(),
      body,
      headers: res.headers(),
      durationMs,
    };
  }

  // ------------------------------- Producers ------------------------------

  async createProducer(
    payload: ProducerCreateRequest,
    traceparent: string = newTraceparent(),
  ): Promise<CoreResponse<ProducerResponse>> {
    const api = await this.api();
    return this.wrap<ProducerResponse>(
      api.post(`${this.prefix()}/producers`, {
        data: payload,
        headers: { traceparent },
      }),
    );
  }

  async getProducer(id: string): Promise<CoreResponse<ProducerResponse>> {
    const api = await this.api();
    return this.wrap<ProducerResponse>(
      api.get(`${this.prefix()}/producers/${encodeURIComponent(id)}`),
    );
  }

  async listProducers(params?: {
    cooperativeId?: string;
    page?: number;
    size?: number;
  }): Promise<CoreResponse<{ items: ProducerResponse[]; page: number; size: number; total?: number }>> {
    const qs = new URLSearchParams();
    if (params?.cooperativeId) qs.set('cooperativeId', params.cooperativeId);
    if (params?.page !== undefined) qs.set('page', String(params.page));
    if (params?.size !== undefined) qs.set('size', String(params.size));
    const api = await this.api();
    return this.wrap(
      api.get(`${this.prefix()}/producers?${qs.toString()}`),
    );
  }

  async patchProducer(
    id: string,
    patch: Partial<ProducerCreateRequest> & { lwwVersion: number },
  ): Promise<CoreResponse<ProducerResponse>> {
    const api = await this.api();
    return this.wrap<ProducerResponse>(
      api.patch(`${this.prefix()}/producers/${encodeURIComponent(id)}`, {
        data: patch,
      }),
    );
  }

  async deleteProducer(id: string): Promise<CoreResponse<unknown>> {
    const api = await this.api();
    return this.wrap(
      api.delete(`${this.prefix()}/producers/${encodeURIComponent(id)}`),
    );
  }

  // -------------------------------- Parcels -------------------------------

  async createParcel(
    payload: ParcelCreateRequest,
  ): Promise<CoreResponse<ParcelResponse>> {
    const api = await this.api();
    return this.wrap<ParcelResponse>(
      api.post(`${this.prefix()}/parcels`, { data: payload }),
    );
  }

  async getParcel(id: string): Promise<CoreResponse<ParcelResponse>> {
    const api = await this.api();
    return this.wrap<ParcelResponse>(
      api.get(`${this.prefix()}/parcels/${encodeURIComponent(id)}`),
    );
  }

  async listParcels(params?: {
    producerId?: string;
    page?: number;
    size?: number;
  }): Promise<CoreResponse<{ items: ParcelResponse[]; page: number; size: number }>> {
    const qs = new URLSearchParams();
    if (params?.producerId) qs.set('producerId', params.producerId);
    if (params?.page !== undefined) qs.set('page', String(params.page));
    if (params?.size !== undefined) qs.set('size', String(params.size));
    const api = await this.api();
    return this.wrap(
      api.get(`${this.prefix()}/parcels?${qs.toString()}`),
    );
  }

  async updatePolygon(
    parcelId: string,
    payload: PolygonUpdateRequest,
  ): Promise<CoreResponse<PolygonResponse>> {
    const api = await this.api();
    return this.wrap<PolygonResponse>(
      api.post(
        `${this.prefix()}/parcels/${encodeURIComponent(parcelId)}/polygon`,
        { data: payload },
      ),
    );
  }

  async getParcelPolygon(
    parcelId: string,
  ): Promise<CoreResponse<PolygonResponse>> {
    const api = await this.api();
    return this.wrap<PolygonResponse>(
      api.get(`${this.prefix()}/parcels/${encodeURIComponent(parcelId)}/polygon`),
    );
  }

  // ------------------------------ Households ------------------------------

  async createHousehold(payload: {
    cooperativeId: string;
    headProducerId?: string;
    yjsUpdate?: string;
  }): Promise<CoreResponse<unknown>> {
    const api = await this.api();
    return this.wrap(
      api.post(`${this.prefix()}/households`, { data: payload }),
    );
  }

  // -------------------------------- Health --------------------------------

  async isReachable(): Promise<boolean> {
    try {
      const api = await request.newContext();
      // ARMAGEDDON :8080 always exposes /health for itself ; the
      // terroir-core /health/ready isn't reachable without going through
      // the routing prefix, so we probe via a `producers` endpoint that
      // unauthenticated will reply 401 (live) instead of timeout (dead).
      // To keep this idempotent + cheap, hit /health on ARMAGEDDON.
      const res = await api.get(`${this.baseURL}/health`);
      return res.ok();
    } catch {
      return false;
    }
  }
}
