// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * Client WebSocket TERROIR mobile-bff `/ws/sync/{producerId}`.
 *
 * Authentification : sub-protocol `bearer.<jwt>` (cf. P1.D — header
 * `Sec-WebSocket-Protocol`) ; le BFF valide le JWT et accepte ou close.
 *
 * Frames texte JSON : `{ type: 'ping'|'pong'|'yjs-update'|'error', ... }`
 * (cf. mobile-bff/src/dto.rs WsFrame).
 *
 * Reconnexion : backoff exponentiel 1s → 2s → 4s → … cap 30s. Heartbeat
 * `ping` côté client toutes les 30s ; si pas de `pong` en 10s, force-close
 * + reconnect.
 */
import Constants from 'expo-constants';

import { loadJwt } from '../auth/jwt-storage';

const DEFAULT_WS_BASE = 'ws://10.0.2.2:8080/api/terroir/mobile-bff';
const HEARTBEAT_INTERVAL_MS = 30_000;
const PONG_TIMEOUT_MS = 10_000;
const RECONNECT_INITIAL_MS = 1_000;
const RECONNECT_MAX_MS = 30_000;

export type WsFrame =
  | { type: 'ping' }
  | { type: 'pong' }
  | { type: 'yjs-update'; parcel_id: string; yjs_delta: string }
  | { type: 'error'; code: string; message: string };

export interface SyncClientCallbacks {
  onYjsUpdate?: (parcelId: string, b64Delta: string) => void;
  onError?: (code: string, message: string) => void;
  onOpen?: () => void;
  onClose?: (code: number, reason: string) => void;
}

function getWsBaseUrl(): string {
  const extra = (Constants.expoConfig?.extra ?? {}) as { wsBaseUrl?: string };
  return extra.wsBaseUrl ?? DEFAULT_WS_BASE;
}

export class SyncWsClient {
  private ws: WebSocket | null = null;
  private heartbeat: ReturnType<typeof setInterval> | null = null;
  private pongTimer: ReturnType<typeof setTimeout> | null = null;
  private reconnectDelay = RECONNECT_INITIAL_MS;
  private closed = false;

  constructor(
    private readonly producerId: string,
    private readonly callbacks: SyncClientCallbacks = {},
  ) {}

  async connect(): Promise<void> {
    if (this.closed) return;
    const jwt = await loadJwt();
    if (!jwt) {
      this.callbacks.onError?.('no_jwt', 'JWT absent — connectez-vous d’abord.');
      return;
    }
    const url = `${getWsBaseUrl()}/ws/sync/${this.producerId}`;
    // RN WebSocket : passer protocols comme 2nd arg → header
    // `Sec-WebSocket-Protocol`.
    const subProtocol = `bearer.${jwt}`;
    try {
      this.ws = new WebSocket(url, subProtocol);
    } catch (err) {
      this.callbacks.onError?.(
        'ws_construct_failed',
        err instanceof Error ? err.message : 'unknown',
      );
      this.scheduleReconnect();
      return;
    }

    this.ws.onopen = () => {
      this.reconnectDelay = RECONNECT_INITIAL_MS;
      this.startHeartbeat();
      this.callbacks.onOpen?.();
    };
    this.ws.onmessage = (ev: WebSocketMessageEvent) => {
      this.handleMessage(typeof ev.data === 'string' ? ev.data : '');
    };
    this.ws.onerror = (ev: Event) => {
      const message = (ev as { message?: string }).message ?? 'ws error';
      this.callbacks.onError?.('ws_error', message);
    };
    this.ws.onclose = (ev: WebSocketCloseEvent) => {
      this.stopHeartbeat();
      this.callbacks.onClose?.(ev.code ?? 0, ev.reason ?? '');
      this.ws = null;
      if (!this.closed) this.scheduleReconnect();
    };
  }

  private handleMessage(raw: string): void {
    let frame: WsFrame;
    try {
      frame = JSON.parse(raw) as WsFrame;
    } catch {
      this.callbacks.onError?.('bad_frame', 'JSON parse failed');
      return;
    }
    switch (frame.type) {
      case 'pong':
        if (this.pongTimer !== null) {
          clearTimeout(this.pongTimer);
          this.pongTimer = null;
        }
        break;
      case 'yjs-update':
        this.callbacks.onYjsUpdate?.(frame.parcel_id, frame.yjs_delta);
        break;
      case 'error':
        this.callbacks.onError?.(frame.code, frame.message);
        break;
      default:
        // ping côté serveur — non attendu, mais on ignore.
        break;
    }
  }

  /**
   * Envoie une mise à jour Yjs (base64) pour un parcelId donné.
   * Si la connexion n'est pas OPEN, la frame est silencieusement perdue
   * (le caller doit aussi enfiler dans `sync-queue` pour persistance).
   */
  sendYjsUpdate(parcelId: string, b64Delta: string): boolean {
    if (this.ws === null || this.ws.readyState !== WebSocket.OPEN) return false;
    const frame: WsFrame = {
      type: 'yjs-update',
      parcel_id: parcelId,
      yjs_delta: b64Delta,
    };
    this.ws.send(JSON.stringify(frame));
    return true;
  }

  private startHeartbeat(): void {
    this.stopHeartbeat();
    this.heartbeat = setInterval(() => {
      if (this.ws !== null && this.ws.readyState === WebSocket.OPEN) {
        this.ws.send(JSON.stringify({ type: 'ping' } satisfies WsFrame));
        this.pongTimer = setTimeout(() => {
          // Pas de pong reçu → on force-close, le onclose triggera reconnect.
          this.ws?.close(4000, 'pong timeout');
        }, PONG_TIMEOUT_MS);
      }
    }, HEARTBEAT_INTERVAL_MS);
  }

  private stopHeartbeat(): void {
    if (this.heartbeat !== null) {
      clearInterval(this.heartbeat);
      this.heartbeat = null;
    }
    if (this.pongTimer !== null) {
      clearTimeout(this.pongTimer);
      this.pongTimer = null;
    }
  }

  private scheduleReconnect(): void {
    const delay = this.reconnectDelay;
    this.reconnectDelay = Math.min(this.reconnectDelay * 2, RECONNECT_MAX_MS);
    setTimeout(() => {
      if (!this.closed) void this.connect();
    }, delay);
  }

  close(): void {
    this.closed = true;
    this.stopHeartbeat();
    this.ws?.close(1000, 'client closing');
    this.ws = null;
  }
}
