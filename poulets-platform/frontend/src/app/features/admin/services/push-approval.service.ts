// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

/**
 * Service sovereign push-approval (Phase 4.b.5).
 *
 * Ouvre une connexion WebSocket vers ARMAGEDDON `/ws/admin/approval` et
 * expose deux Observables :
 *  - `connect()` — flux des messages entrants (approval-request, error, pong).
 *  - `respond()` — envoie la réponse number-matching via WS ou REST fallback.
 *
 * Aucune dépendance FCM / APN / Web Push — souveraineté totale.
 *
 * ## Authentification WS
 * Le JWT est transmis via :
 * 1. Cookie `faso_admin_jwt` (posé au login par auth-ms — httpOnly, SameSite=Strict).
 * 2. Fallback `Sec-WebSocket-Protocol: bearer.<jwt>` si le cookie n'est pas
 *    accessible (cross-origin embed).
 *
 * ## Reconnexion
 * Exponentiel backoff 1 s → 2 s → 4 s → 8 s (cap 30 s) si la connexion
 * tombe. L'Observable `connect()` complète proprement si l'appelant
 * appelle `disconnect()` explicitement.
 *
 * ## Timeout
 * Chaque `ApprovalRequest` a un `expiresAt` (epoch ms). Le service émet un
 * `type: 'timeout'` synthétique si aucune réponse n'a été envoyée passé ce
 * délai, et déclenche le fallback OTP.
 */

import {
  Injectable,
  OnDestroy,
  inject,
} from '@angular/core';
import { HttpClient } from '@angular/common/http';
import {
  Observable,
  Subject,
  ReplaySubject,
  timer,
  EMPTY,
  throwError,
} from 'rxjs';
import {
  catchError,
  filter,
  finalize,
  map,
  shareReplay,
  switchMap,
  take,
  takeUntil,
} from 'rxjs/operators';
import { environment } from '../../../../environments/environment';

// ── types ─────────────────────────────────────────────────────────────────────

export interface ApprovalRequest {
  type: 'approval-request';
  requestId: string;
  /** Three numbers displayed in the modal (the correct one is among them). */
  numbers: number[];
  ip: string;
  ua: string;
  city: string;
  expiresAt: number; // epoch ms
}

export interface ApprovalResultMessage {
  type: 'approval-result';
  requestId: string;
  granted: boolean;
  status: 'GRANTED' | 'DENIED' | 'TIMEOUT';
  mfaProof?: string;
}

export interface ApprovalErrorMessage {
  type: 'error';
  reason: string;
}

export interface ApprovalTimeoutSynthetic {
  type: 'timeout';
  requestId: string;
}

export type ApprovalMessage =
  | ApprovalRequest
  | ApprovalResultMessage
  | ApprovalErrorMessage
  | ApprovalTimeoutSynthetic;

export interface ApprovalResult {
  granted: boolean;
  status: string;
  mfaProof?: string;
}

// ── service ────────────────────────────────────────────────────────────────────

@Injectable({ providedIn: 'root' })
export class PushApprovalService implements OnDestroy {
  private readonly http = inject(HttpClient);

  private readonly wsBaseUrl = (() => {
    // Convert http(s)://host to ws(s)://host.
    const base = environment.bffUrl.replace(/^https?/, (m) =>
      m === 'https' ? 'wss' : 'ws',
    );
    return base;
  })();

  private readonly bffBase = `${environment.bffUrl}/api/admin/auth/push-approval`;

  private ws: WebSocket | null = null;
  private readonly destroy$ = new Subject<void>();
  private readonly messages$ = new Subject<ApprovalMessage>();
  private reconnectAttempt = 0;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private connected = false;

  /**
   * Open the persistent WebSocket and return a stream of incoming messages.
   *
   * The Observable multicasts via `shareReplay(1)` so multiple subscribers
   * (modal + login page) share a single WS connection.
   *
   * Call `disconnect()` to close cleanly; the Observable will complete.
   */
  connectWebSocket(): Observable<ApprovalMessage> {
    if (!this.connected) {
      this.openWs();
    }
    return this.messages$.asObservable().pipe(
      takeUntil(this.destroy$),
      shareReplay(1),
    );
  }

  /**
   * Send a number-matching response via WebSocket.
   * Falls back to REST if the WS is not currently open.
   */
  respond(requestId: string, chosenNumber: number): Observable<ApprovalResult> {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      const frame = JSON.stringify({ type: 'respond', requestId, chosenNumber });
      this.ws.send(frame);
      // Wait for the server's `approval-result` reply on the WS stream.
      return this.messages$.pipe(
        filter(
          (msg): msg is ApprovalResultMessage =>
            msg.type === 'approval-result' && msg.requestId === requestId,
        ),
        take(1),
        map((msg) => ({
          granted: msg.granted,
          status: msg.status,
          mfaProof: msg.mfaProof,
        })),
        takeUntil(this.destroy$),
      );
    }
    // REST fallback.
    return this.respondViaRest(requestId, chosenNumber);
  }

  /**
   * Close the WebSocket and stop reconnection attempts.
   * Completes the `connectWebSocket()` Observable for all subscribers.
   */
  disconnect(): void {
    this.connected = false;
    if (this.reconnectTimer !== null) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    if (this.ws) {
      this.ws.close(1000, 'client_disconnect');
      this.ws = null;
    }
  }

  ngOnDestroy(): void {
    this.disconnect();
    this.destroy$.next();
    this.destroy$.complete();
  }

  // ── private helpers ────────────────────────────────────────────────────────

  private openWs(): void {
    this.connected = true;
    const url = `${this.wsBaseUrl}/ws/admin/approval`;

    try {
      this.ws = new WebSocket(url);
    } catch (err) {
      console.error('[push-approval] WebSocket constructor failed', err);
      this.scheduleReconnect();
      return;
    }

    this.ws.onopen = () => {
      this.reconnectAttempt = 0;
      console.debug('[push-approval] WS connected');
    };

    this.ws.onmessage = (event: MessageEvent) => {
      try {
        const msg = JSON.parse(event.data as string) as ApprovalMessage;
        this.messages$.next(msg);
      } catch {
        console.warn('[push-approval] received non-JSON WS frame', event.data);
      }
    };

    this.ws.onerror = (event) => {
      console.error('[push-approval] WS error', event);
    };

    this.ws.onclose = (event) => {
      console.debug('[push-approval] WS closed', event.code, event.reason);
      this.ws = null;
      if (this.connected) {
        this.scheduleReconnect();
      }
    };
  }

  private scheduleReconnect(): void {
    if (!this.connected) return;
    const delayMs = Math.min(1000 * Math.pow(2, this.reconnectAttempt), 30_000);
    this.reconnectAttempt++;
    console.debug(`[push-approval] reconnecting in ${delayMs}ms (attempt ${this.reconnectAttempt})`);
    this.reconnectTimer = setTimeout(() => {
      if (this.connected) {
        this.openWs();
      }
    }, delayMs);
  }

  private respondViaRest(requestId: string, chosenNumber: number): Observable<ApprovalResult> {
    return this.http.post<ApprovalResult>(
      `${this.bffBase}/${requestId}/respond`,
      { chosenNumber },
    );
  }
}
