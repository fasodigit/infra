// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * EudrClient — wrapper REST `terroir-eudr` (validation EUDR + DDS).
 *
 * Couvre les endpoints P1.3 documentés dans
 * `INFRA/terroir/docs/ULTRAPLAN-TERROIR-2026-04-30.md` §6 P1.3 :
 *   - POST /eudr/validate                 (Hansen GFC overlap, cache MISS/HIT)
 *   - POST /eudr/dds                      (generate DDS PDF + payload)
 *   - POST /eudr/dds/{id}/sign            (Vault PKI signature)
 *   - POST /eudr/dds/{id}/submit          (TRACES NT mock)
 *   - GET  /eudr/dds/{id}/download        (PDF binary)
 *   - GET  /eudr/parcels/{id}/validations (history)
 *
 * Le routage passe **toujours** par ARMAGEDDON (`/api/terroir/eudr/*`).
 * Authentification : `X-Tenant-Slug` (M2M loopback) ou bearer JWT.
 *
 * Pas de mocks côté Playwright : seul le provider TRACES NT est mocké
 * côté serveur (cf. EUDR_TRACES_NT_URL=stub://). Cette spec assume que le
 * stub renvoie `200 OK` + `reference: "MOCK-TRACES-NT-..."`.
 */
import {
  request,
  type APIRequestContext,
  type APIResponse,
} from '@playwright/test';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type ValidationStatus = 'VALIDATED' | 'REJECTED' | 'ESCALATED';

export interface ValidateRequest {
  parcelId: string;
  polygonGeoJson: Record<string, unknown>;
}

export interface ValidationResponse {
  validationId: string;
  parcelId: string;
  status: ValidationStatus;
  evidenceUrl?: string;
  ddsDraftId?: string;
  deforestationOverlapHa: number;
  datasetVersion: string;
  polygonHash: string;
  cacheStatus?: 'HIT' | 'MISS' | string;
  computedAt: string;
}

export interface GenerateDdsRequest {
  validationId: string;
  operatorEori?: string;
  hsCode: string;
  quantity: number;
  unit: string;
  countryIso2: string;
  harvestPeriod: string;
}

export interface DdsResponse {
  ddsId: string;
  validationId: string;
  parcelId: string;
  status: string;
  operatorEori: string;
  hsCode: string;
  countryIso2: string;
  evidenceUrl?: string;
  payloadSha256: string;
  createdAt: string;
}

export interface DdsSignResponse {
  ddsId: string;
  signatureFingerprint: string;
  status: string;
  signedAt: string;
}

export interface DdsSubmitResponse {
  ddsId: string;
  status: string;
  tracesNtRef?: string;
  attemptNo: number;
}

export interface EudrError {
  error: string;
  message?: string;
  details?: unknown;
}

export interface EudrResponse<T> {
  status: number;
  body: T | EudrError;
  headers: Record<string, string>;
  durationMs: number;
}

export interface EudrClientOptions {
  baseURL?: string;
  tenantSlug?: string;
  bearer?: string;
}

// ---------------------------------------------------------------------------
// Helpers — synthetic Burkina Faso polygons (clean + deforested)
// ---------------------------------------------------------------------------

/**
 * Produit un GeoJSON Feature Polygon ~300m x 300m centré sur (lon, lat),
 * encodé en GeoJSON RFC 7946 (longitude, latitude). Coordonnées auto-fermées.
 *
 * Les coordonnées suivantes pointent typiquement vers une zone agricole
 * Boucle du Mouhoun (BF) loin des hotspots de déforestation Hansen GFC :
 *   centre par défaut = (3.0270 W, 12.5210 N) ≈ Tougan, Sourou.
 */
export function bfCleanPolygon(
  centerLat = 12.521,
  centerLon = -3.027,
): Record<string, unknown> {
  const dLat = 0.0027; // ~300m
  const dLon = 0.0028;
  const ring = [
    [centerLon - dLon, centerLat - dLat],
    [centerLon + dLon, centerLat - dLat],
    [centerLon + dLon, centerLat + dLat],
    [centerLon - dLon, centerLat + dLat],
    [centerLon - dLon, centerLat - dLat],
  ];
  return {
    type: 'Feature',
    properties: { source: 'e2e-fixture', kind: 'clean-bf-bm' },
    geometry: {
      type: 'Polygon',
      coordinates: [ring],
    },
  };
}

/**
 * Produit un GeoJSON Polygon synthétique sur une zone forte loss
 * post-2020 d'après Hansen GFC. La zone visée est l'enclave forestière
 * de Comoé (Côte d'Ivoire) — utilisée ici comme "ground-truth déforesté"
 * pour les tests P1 (ne reflète pas la production BF, c'est une garantie
 * que le validateur détecte une overlap > seuil).
 */
export function deforestedPolygon(): Record<string, unknown> {
  // Centre dans une zone Hansen lossyear≥2021 (placeholder fixture).
  const centerLat = 6.31;
  const centerLon = -3.66;
  const dLat = 0.005;
  const dLon = 0.005;
  const ring = [
    [centerLon - dLon, centerLat - dLat],
    [centerLon + dLon, centerLat - dLat],
    [centerLon + dLon, centerLat + dLat],
    [centerLon - dLon, centerLat + dLat],
    [centerLon - dLon, centerLat - dLat],
  ];
  return {
    type: 'Feature',
    properties: { source: 'e2e-fixture', kind: 'deforested-synth' },
    geometry: {
      type: 'Polygon',
      coordinates: [ring],
    },
  };
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

export class EudrClient {
  private readonly baseURL: string;
  private readonly tenantSlug: string;
  private readonly bearer?: string;

  constructor(opts: EudrClientOptions = {}) {
    this.baseURL =
      opts.baseURL ?? process.env.TERROIR_GATEWAY_URL ?? 'http://localhost:8080';
    this.tenantSlug = opts.tenantSlug ?? process.env.TERROIR_TENANT_SLUG ?? 't_pilot';
    this.bearer = opts.bearer;
  }

  private prefix(): string {
    return `${this.baseURL}/api/terroir/eudr`;
  }

  private async api(): Promise<APIRequestContext> {
    const headers: Record<string, string> = {
      'content-type': 'application/json',
      accept: 'application/json',
    };
    if (this.bearer) {
      headers.Authorization = `Bearer ${this.bearer}`;
    } else {
      headers['X-Tenant-Slug'] = this.tenantSlug;
    }
    return request.newContext({ extraHTTPHeaders: headers });
  }

  private async wrap<T>(p: Promise<APIResponse>): Promise<EudrResponse<T>> {
    const start = Date.now();
    const res = await p;
    const durationMs = Date.now() - start;
    let body: T | EudrError;
    try {
      body = (await res.json()) as T | EudrError;
    } catch {
      body = { error: 'invalid_json' } as EudrError;
    }
    return {
      status: res.status(),
      body,
      headers: res.headers(),
      durationMs,
    };
  }

  async validate(
    req: ValidateRequest,
  ): Promise<EudrResponse<ValidationResponse>> {
    const api = await this.api();
    return this.wrap<ValidationResponse>(
      api.post(`${this.prefix()}/validate`, { data: req }),
    );
  }

  async generateDds(
    req: GenerateDdsRequest,
  ): Promise<EudrResponse<DdsResponse>> {
    const api = await this.api();
    return this.wrap<DdsResponse>(
      api.post(`${this.prefix()}/dds`, { data: req }),
    );
  }

  async signDds(
    ddsId: string,
    operatorEori?: string,
  ): Promise<EudrResponse<DdsSignResponse>> {
    const api = await this.api();
    return this.wrap<DdsSignResponse>(
      api.post(`${this.prefix()}/dds/${encodeURIComponent(ddsId)}/sign`, {
        data: operatorEori ? { operatorEori } : {},
      }),
    );
  }

  async submitDds(
    ddsId: string,
  ): Promise<EudrResponse<DdsSubmitResponse>> {
    const api = await this.api();
    return this.wrap<DdsSubmitResponse>(
      api.post(`${this.prefix()}/dds/${encodeURIComponent(ddsId)}/submit`, {
        data: {},
      }),
    );
  }

  /** Renvoie {bytes, contentType, signatureFingerprint?} — pas wrap json. */
  async downloadDds(ddsId: string): Promise<{
    status: number;
    bytes: Buffer;
    contentType: string;
    contentDisposition?: string;
  }> {
    const api = await this.api();
    const res = await api.get(
      `${this.prefix()}/dds/${encodeURIComponent(ddsId)}/download`,
    );
    const bytes = Buffer.from(await res.body());
    return {
      status: res.status(),
      bytes,
      contentType: res.headers()['content-type'] ?? '',
      contentDisposition: res.headers()['content-disposition'],
    };
  }

  async listValidations(parcelId: string): Promise<
    EudrResponse<{ items: ValidationResponse[] }>
  > {
    const api = await this.api();
    return this.wrap(
      api.get(
        `${this.prefix()}/parcels/${encodeURIComponent(parcelId)}/validations`,
      ),
    );
  }

  async isReachable(): Promise<boolean> {
    try {
      const api = await request.newContext();
      const res = await api.get(`${this.baseURL}/health`);
      return res.ok();
    } catch {
      return false;
    }
  }
}
