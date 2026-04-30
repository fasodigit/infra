// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * MobileBffClient — wrapper REST + WebSocket pour `terroir-mobile-bff`.
 *
 * Couvre les endpoints P1.5 documentés dans
 * `INFRA/terroir/docs/ULTRAPLAN-TERROIR-2026-04-30.md` §6 P1.5 :
 *   - GET  /m/producers          (compact list)
 *   - GET  /m/parcels            (compact list, WKT geom)
 *   - POST /m/sync/batch         (batch CRDT/LWW updates, idempotency KAYA)
 *   - GET  /ws/sync/{producerId} (WebSocket sub-protocol `bearer.<jwt>`)
 *
 * Le routage passe **toujours** par ARMAGEDDON :
 *   - REST : `/api/terroir/mobile-bff/m/...`
 *   - WS   : `/ws/terroir/sync/{producerId}`
 *
 * Pas de mocks : real backend en cycle-fix GREEN.
 *
 * Note importante WS : Playwright n'expose PAS de helper WebSocket fiable
 * pour Node 22+ avec sub-protocol. On utilise donc le WebSocket browser API
 * via `page.evaluate` quand on a une `page`, sinon on importe `ws` (npm)
 * dynamiquement (peer dep). Si `ws` est absent, l'helper renvoie
 * `WS_LIB_UNAVAILABLE`.
 */
import {
  request,
  type APIRequestContext,
  type APIResponse,
} from '@playwright/test';
import { randomUUID } from 'node:crypto';

// ---------------------------------------------------------------------------
// Types — alignés sur INFRA/terroir/mobile-bff/src/dto.rs
// ---------------------------------------------------------------------------

export interface CompactProducer {
  id: string;
  cooperativeId: string;
  fullName: string;
  primaryCrop?: string;
  updatedAt: string;
  lwwVersion: number;
}

export interface CompactParcel {
  id: string;
  producerId: string;
  cropType?: string;
  surfaceHectares?: number;
  geomWkt?: string;
  updatedAt: string;
  lwwVersion: number;
}

export interface MobilePage<T> {
  items: T[];
  page: number;
  size: number;
}

/** Discriminator on the wire = `type` field, kebab-case. */
export type SyncItem =
  | {
      type: 'parcel-polygon-update';
      parcelId: string;
      yjsDelta: string;
    }
  | {
      type: 'agronomy-note-update';
      parcelId: string;
      noteId?: string;
      yjsDelta: string;
    }
  | {
      type: 'producer-update';
      producerId: string;
      lwwVersion: number;
      patch: Record<string, unknown>;
    }
  | {
      type: 'parcel-update';
      parcelId: string;
      lwwVersion: number;
      patch: Record<string, unknown>;
    }
  | {
      type: 'household-update';
      householdId: string;
      yjsDelta: string;
    };

export interface SyncBatchRequest {
  batchId: string;
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
  batchId: string;
  acks: SyncItemAck[];
}

export interface BffError {
  error: string;
  message?: string;
}

export interface BffResponse<T> {
  status: number;
  body: T | BffError;
  headers: Record<string, string>;
  durationMs: number;
}

export interface MobileBffClientOptions {
  baseURL?: string;
  tenantSlug?: string;
  bearer?: string;
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

export class MobileBffClient {
  private readonly baseURL: string;
  private readonly tenantSlug: string;
  private readonly bearer?: string;

  constructor(opts: MobileBffClientOptions = {}) {
    this.baseURL =
      opts.baseURL ?? process.env.TERROIR_GATEWAY_URL ?? 'http://localhost:8080';
    this.tenantSlug = opts.tenantSlug ?? process.env.TERROIR_TENANT_SLUG ?? 't_pilot';
    this.bearer = opts.bearer;
  }

  private prefix(): string {
    return `${this.baseURL}/api/terroir/mobile-bff/m`;
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

  private async wrap<T>(p: Promise<APIResponse>): Promise<BffResponse<T>> {
    const start = Date.now();
    const res = await p;
    const durationMs = Date.now() - start;
    let body: T | BffError;
    try {
      body = (await res.json()) as T | BffError;
    } catch {
      body = { error: 'invalid_json' } as BffError;
    }
    return {
      status: res.status(),
      body,
      headers: res.headers(),
      durationMs,
    };
  }

  async listProducers(params?: {
    cooperativeId?: string;
    page?: number;
    size?: number;
  }): Promise<BffResponse<MobilePage<CompactProducer>>> {
    const qs = new URLSearchParams();
    if (params?.cooperativeId) qs.set('cooperativeId', params.cooperativeId);
    if (params?.page !== undefined) qs.set('page', String(params.page));
    if (params?.size !== undefined) qs.set('size', String(params.size));
    const api = await this.api();
    return this.wrap(api.get(`${this.prefix()}/producers?${qs.toString()}`));
  }

  async listParcels(params?: {
    producerId?: string;
    page?: number;
    size?: number;
  }): Promise<BffResponse<MobilePage<CompactParcel>>> {
    const qs = new URLSearchParams();
    if (params?.producerId) qs.set('producerId', params.producerId);
    if (params?.page !== undefined) qs.set('page', String(params.page));
    if (params?.size !== undefined) qs.set('size', String(params.size));
    const api = await this.api();
    return this.wrap(api.get(`${this.prefix()}/parcels?${qs.toString()}`));
  }

  async syncBatch(
    batch: SyncBatchRequest,
  ): Promise<BffResponse<SyncBatchResponse>> {
    const api = await this.api();
    return this.wrap<SyncBatchResponse>(
      api.post(`${this.prefix()}/sync/batch`, { data: batch }),
    );
  }

  /**
   * Génère N items synthétiques répartis comme demandé pour la spec
   * `terroir-agent-offline-sync-roundtrip.spec.ts` :
   *   {producerUpdates, parcelUpdates, polygonUpdates}.
   * Chaque item utilise un parcel/producer existant fourni en paramètre.
   *
   * Note : `mobile-bff` impose `SYNC_BATCH_MAX_ITEMS = 100` côté serveur,
   * la spec peut donc envoyer 50 items en un seul batch sans split.
   */
  static makeSyntheticBatch(opts: {
    producerIds: string[];
    parcelIds: string[];
    counts?: {
      producerUpdates?: number;
      parcelUpdates?: number;
      polygonUpdates?: number;
    };
  }): SyncBatchRequest {
    const c = {
      producerUpdates: 5,
      parcelUpdates: 5,
      polygonUpdates: 40,
      ...(opts.counts ?? {}),
    };
    const items: SyncItem[] = [];
    for (let i = 0; i < c.producerUpdates; i++) {
      const pid = opts.producerIds[i % opts.producerIds.length]!;
      items.push({
        type: 'producer-update',
        producerId: pid,
        lwwVersion: 1,
        patch: { primaryCrop: i % 2 === 0 ? 'coton' : 'mais' },
      });
    }
    for (let i = 0; i < c.parcelUpdates; i++) {
      const pid = opts.parcelIds[i % opts.parcelIds.length]!;
      items.push({
        type: 'parcel-update',
        parcelId: pid,
        lwwVersion: 1,
        patch: { surfaceHectares: 0.5 + i * 0.1 },
      });
    }
    for (let i = 0; i < c.polygonUpdates; i++) {
      const pid = opts.parcelIds[i % opts.parcelIds.length]!;
      // Empty Yjs delta is invalid — synthesise a 1-byte b64 placeholder
      // (server validates structure, not content, in P1).
      items.push({
        type: 'parcel-polygon-update',
        parcelId: pid,
        yjsDelta: Buffer.from([0]).toString('base64'),
      });
    }
    return { batchId: randomUUID(), items };
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

  /**
   * Open a real browser-grade WebSocket via the optional `ws` package.
   * Returns null when `ws` is absent — callers should skip the WS portion
   * of the spec rather than failing.
   */
  async openSyncSocket(producerId: string): Promise<{
    sendText: (msg: string) => void;
    nextMessage: (timeoutMs?: number) => Promise<string>;
    close: () => void;
  } | null> {
    let WebSocketCtor: unknown;
    try {
      // Dynamic import (runtime-only — `ws` is optional).
      const wsPath = 'ws';
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const mod: any = await import(/* @vite-ignore */ wsPath);
      WebSocketCtor = mod.default ?? mod.WebSocket ?? mod;
    } catch {
      return null;
    }
    const wsURL =
      this.baseURL.replace(/^http/, 'ws') +
      `/ws/terroir/sync/${encodeURIComponent(producerId)}`;
    const subprotocol = this.bearer ? `bearer.${this.bearer}` : 'bearer.anonymous';
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const ctor = WebSocketCtor as new (url: string, protocols: string) => any;
    const socket = new ctor(wsURL, subprotocol);
    const incoming: string[] = [];
    let resolve: ((s: string) => void) | null = null;
    socket.on('message', (raw: Buffer) => {
      const text = raw.toString('utf8');
      if (resolve) {
        resolve(text);
        resolve = null;
      } else {
        incoming.push(text);
      }
    });
    return {
      sendText: (msg: string): void => socket.send(msg),
      nextMessage: (timeoutMs = 5_000): Promise<string> => {
        return new Promise<string>((res, rej) => {
          if (incoming.length > 0) {
            res(incoming.shift()!);
            return;
          }
          resolve = res;
          setTimeout(() => rej(new Error('WS message timeout')), timeoutMs);
        });
      },
      close: (): void => socket.close(),
    };
  }
}
