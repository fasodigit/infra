// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * TenantAdminClient — wrapper du service `terroir-admin :9904` (REST loopback).
 *
 * Couvre les endpoints P0.C documentés dans
 * `INFRA/terroir/docs/ULTRAPLAN-TERROIR-2026-04-30.md` §4 P0.3 :
 *   - POST /admin/tenants
 *   - GET  /admin/tenants
 *   - GET  /admin/tenants/:slug
 *   - POST /admin/tenants/:slug/suspend
 *
 * Toutes les requêtes propagent un en-tête `traceparent` W3C pour permettre
 * la corrélation OTel/Tempo dans Jaeger lors d'un fail E2E.
 *
 * Pas de mocks : la classe parle au service réel exposé par `cycle-fix`.
 */
import { request, type APIRequestContext, type APIResponse } from '@playwright/test';

export interface CreateTenantRequest {
  slug: string;
  legal_name: string;
  country_iso2: string;
  region: string;
  primary_crop: string;
  contact_email?: string;
  contact_phone?: string;
}

export interface TenantRecord {
  id: string;
  slug: string;
  legal_name: string;
  country_iso2: string;
  region: string;
  primary_crop: string;
  status: 'ACTIVE' | 'SUSPENDED' | 'PENDING';
  schema_name: string;
  audit_schema_name: string;
  created_at?: string;
  updated_at?: string;
}

export interface TenantAdminError {
  error: string;
  message?: string;
  details?: unknown;
}

export interface TenantAdminResponse<T> {
  status: number;
  body: T | TenantAdminError;
  headers: Record<string, string>;
  durationMs: number;
}

/** Génère un `traceparent` W3C aléatoire (sample bit = 01). */
export function newTraceparent(): string {
  const hex = (n: number): string =>
    Array.from({ length: n }, () =>
      Math.floor(Math.random() * 16).toString(16),
    ).join('');
  return `00-${hex(32)}-${hex(16)}-01`;
}

export class TenantAdminClient {
  private readonly baseURL: string;

  constructor(baseURL?: string) {
    this.baseURL = baseURL ?? process.env.TERROIR_ADMIN_URL ?? 'http://localhost:9904';
  }

  private async api(): Promise<APIRequestContext> {
    return request.newContext({
      extraHTTPHeaders: {
        'content-type': 'application/json',
        accept: 'application/json',
      },
    });
  }

  private async wrap<T>(p: Promise<APIResponse>): Promise<TenantAdminResponse<T>> {
    const start = Date.now();
    const res = await p;
    const durationMs = Date.now() - start;
    let body: T | TenantAdminError;
    try {
      body = (await res.json()) as T | TenantAdminError;
    } catch {
      body = { error: 'invalid_json' } as TenantAdminError;
    }
    return {
      status: res.status(),
      body,
      headers: res.headers(),
      durationMs,
    };
  }

  async createTenant(
    payload: CreateTenantRequest,
    traceparent: string = newTraceparent(),
  ): Promise<TenantAdminResponse<TenantRecord>> {
    const api = await this.api();
    return this.wrap<TenantRecord>(
      api.post(`${this.baseURL}/admin/tenants`, {
        data: payload,
        headers: { traceparent },
      }),
    );
  }

  async getTenant(
    slug: string,
    traceparent: string = newTraceparent(),
  ): Promise<TenantAdminResponse<TenantRecord>> {
    const api = await this.api();
    return this.wrap<TenantRecord>(
      api.get(`${this.baseURL}/admin/tenants/${encodeURIComponent(slug)}`, {
        headers: { traceparent },
      }),
    );
  }

  async listTenants(
    traceparent: string = newTraceparent(),
  ): Promise<TenantAdminResponse<{ tenants: TenantRecord[] }>> {
    const api = await this.api();
    // Server uses keyset-paginated shape { items, next_cursor, limit };
    // we wrap to the legacy { tenants } interface used by specs.
    const raw = await this.wrap<{ items: TenantRecord[]; next_cursor?: string; limit: number }>(
      api.get(`${this.baseURL}/admin/tenants`, {
        headers: { traceparent },
      }),
    );
    let body: { tenants: TenantRecord[] } | TenantAdminError;
    if ('items' in (raw.body as { items?: TenantRecord[] })) {
      body = { tenants: (raw.body as { items: TenantRecord[] }).items };
    } else {
      body = raw.body as TenantAdminError;
    }
    return {
      status: raw.status,
      body,
      headers: raw.headers,
      durationMs: raw.durationMs,
    };
  }

  async suspendTenant(
    slug: string,
    traceparent: string = newTraceparent(),
  ): Promise<TenantAdminResponse<TenantRecord>> {
    const api = await this.api();
    return this.wrap<TenantRecord>(
      api.post(`${this.baseURL}/admin/tenants/${encodeURIComponent(slug)}/suspend`, {
        headers: { traceparent },
        data: {},
      }),
    );
  }

  async isReachable(): Promise<boolean> {
    try {
      const api = await this.api();
      // terroir-admin exposes /health/ready (CLAUDE.md §10 readiness probe).
      const res = await api.get(`${this.baseURL}/health/ready`);
      return res.ok();
    } catch {
      return false;
    }
  }
}
