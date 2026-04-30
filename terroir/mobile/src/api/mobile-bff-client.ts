// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Wrapper haut niveau vers terroir-mobile-bff (:8833) via ARMAGEDDON :8080.
 *
 * S'appuie sur `client.ts` (request, traceparent W3C, JWT auto). Expose des
 * fonctions typées par endpoint P1.D :
 *   - GET  /m/producers?cooperativeId=&page=&size=
 *   - GET  /m/parcels?producerId=&page=&size=
 *   - POST /m/sync/batch
 *
 * Tous les chemins respectent le préfixe `/m/*` côté BFF (cf. mobile-bff/src/routes.rs).
 */
import { apiClient } from './client';
import type { UUID } from './types';

// ---------------------------------------------------------------------------
// DTOs miroirs des Rust serde structs (cf. mobile-bff/src/dto.rs)
// ---------------------------------------------------------------------------

export interface CompactProducer {
  id: UUID;
  cooperativeId: UUID;
  fullName: string;
  primaryCrop?: string;
  updatedAt: string; // ISO 8601
  lwwVersion: number;
}

export interface CompactParcel {
  id: UUID;
  producerId: UUID;
  cropType?: string;
  surfaceHectares?: number;
  geomWkt?: string; // WKT polygon
  updatedAt: string;
  lwwVersion: number;
}

export interface MobilePageResponse<T> {
  items: T[];
  page: number;
  size: number;
}

// ---------------------------------------------------------------------------
// Sync batch (mirroring SyncItem enum tagged "type" kebab-case)
// ---------------------------------------------------------------------------

export type SyncItem =
  | {
      type: 'parcel-polygon-update';
      parcelId: UUID;
      yjsDelta: string; // base64 Yjs v1 update
    }
  | {
      type: 'agronomy-note-update';
      parcelId: UUID;
      noteId?: UUID;
      yjsDelta: string;
    }
  | {
      type: 'producer-update';
      producerId: UUID;
      lwwVersion: number;
      patch: Record<string, unknown>;
    }
  | {
      type: 'parcel-update';
      parcelId: UUID;
      lwwVersion: number;
      patch: Record<string, unknown>;
    }
  | {
      type: 'household-update';
      householdId: UUID;
      yjsDelta: string;
    };

export interface SyncBatchRequest {
  batchId: UUID;
  items: SyncItem[];
}

export interface SyncItemAck {
  index: number;
  status: 'ok' | 'error';
  serverVersion?: number;
  error?: string;
  message?: string;
}

export interface SyncBatchResponse {
  batchId: UUID;
  acks: SyncItemAck[];
}

// ---------------------------------------------------------------------------
// Endpoints
// ---------------------------------------------------------------------------

export interface ListProducersParams {
  cooperativeId?: UUID;
  page?: number;
  size?: number;
}

export async function listProducers(
  params: ListProducersParams = {},
): Promise<MobilePageResponse<CompactProducer>> {
  const search = new URLSearchParams();
  if (params.cooperativeId) search.set('cooperativeId', params.cooperativeId);
  if (typeof params.page === 'number') search.set('page', String(params.page));
  if (typeof params.size === 'number') search.set('size', String(params.size));
  const qs = search.toString();
  const path = qs.length > 0 ? `/m/producers?${qs}` : '/m/producers';
  return apiClient.get<MobilePageResponse<CompactProducer>>(path);
}

export interface ListParcelsParams {
  producerId?: UUID;
  cooperativeId?: UUID;
  page?: number;
  size?: number;
}

export async function listParcels(
  params: ListParcelsParams = {},
): Promise<MobilePageResponse<CompactParcel>> {
  const search = new URLSearchParams();
  if (params.producerId) search.set('producerId', params.producerId);
  if (params.cooperativeId) search.set('cooperativeId', params.cooperativeId);
  if (typeof params.page === 'number') search.set('page', String(params.page));
  if (typeof params.size === 'number') search.set('size', String(params.size));
  const qs = search.toString();
  const path = qs.length > 0 ? `/m/parcels?${qs}` : '/m/parcels';
  return apiClient.get<MobilePageResponse<CompactParcel>>(path);
}

export async function postSyncBatch(req: SyncBatchRequest): Promise<SyncBatchResponse> {
  return apiClient.post<SyncBatchResponse>('/m/sync/batch', req);
}
