// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { HttpClient } from '@angular/common/http';
import { Injectable, Signal, computed, inject, signal } from '@angular/core';
import { Observable, Subject, firstValueFrom } from 'rxjs';

import { environment } from '../../../../environments/environment';

/**
 * Step-up auth (Phase 4.b.7 — SECURITY-HARDENING-PLAN-2026-04-30 §4 Tier 4).
 *
 * Coordonne l'ouverture d'une session step-up et la vérification de la preuve
 * (PassKey / push-approval / TOTP / OTP). Le service expose un signal
 * `pending` que `StepUpInterceptor` lit pour présenter le modal
 * `<faso-step-up-guard>` quand l'upstream renvoie `401 + step_up_required`.
 */

export type StepUpMethodWire = 'passkey' | 'push-approval' | 'totp' | 'otp';

export interface StepUpRequiredPayload {
  readonly error: 'step_up_required';
  readonly methods_available: readonly StepUpMethodWire[];
  readonly step_up_session_id: string;
  readonly expires_at: string;
}

export interface StepUpBeginResponse {
  readonly sessionId: string;
  readonly allowedMethods: readonly StepUpMethodWire[];
  readonly expiresAt: string;
}

export interface StepUpVerifyResponse {
  readonly stepUpToken: string;
  readonly method: StepUpMethodWire;
  readonly expiresInSeconds: number;
}

export interface StepUpStatus {
  readonly sessionId: string;
  readonly status: 'PENDING' | 'VERIFIED' | 'FAILED';
}

/** Session courante poussée par {@link StepUpInterceptor} dans le service. */
export interface StepUpPending {
  readonly sessionId: string;
  readonly methods: readonly StepUpMethodWire[];
  readonly expiresAt: string;
  /** URL originale qui a déclenché le 401. */
  readonly retryUrl: string;
}

@Injectable({ providedIn: 'root' })
export class StepUpService {
  private readonly http = inject(HttpClient);
  private readonly base = `${environment.bffUrl}/api/admin/auth/step-up`;

  /** Active step-up demand — consumed by the guard component. */
  private readonly pending = signal<StepUpPending | null>(null);
  readonly currentPending: Signal<StepUpPending | null> = this.pending.asReadonly();
  readonly hasPending = computed<boolean>(() => this.pending() !== null);

  /**
   * Stream émis par le composant guard à la fin du verify : `{sessionId,token}`
   * (token = null si l'utilisateur a annulé). L'intercepteur HTTP s'y abonne
   * pour rejouer la requête originale.
   */
  readonly tokenStream = new Subject<{ sessionId: string; token: string | null }>();

  /**
   * Test if a HTTP body looks like the auth-ms `step_up_required` envelope.
   */
  static isStepUpRequired(body: unknown): body is StepUpRequiredPayload {
    if (body === null || typeof body !== 'object') return false;
    const b = body as Record<string, unknown>;
    return (
      b['error'] === 'step_up_required' &&
      Array.isArray(b['methods_available']) &&
      typeof b['step_up_session_id'] === 'string'
    );
  }

  /**
   * Called by the interceptor when a 401 step_up_required arrives. Stores
   * the demand so that the guard component can react.
   */
  registerPending(payload: StepUpRequiredPayload, retryUrl: string): void {
    this.pending.set({
      sessionId: payload.step_up_session_id,
      methods: payload.methods_available,
      expiresAt: payload.expires_at,
      retryUrl,
    });
  }

  /** Clear after success / cancel. */
  clearPending(): void {
    this.pending.set(null);
  }

  /** Called by the guard component on successful verify (token) or cancel (null). */
  publishToken(sessionId: string, token: string | null): void {
    this.tokenStream.next({ sessionId, token });
    this.pending.set(null);
  }

  begin(requestedFor: string): Observable<StepUpBeginResponse> {
    return this.http.post<StepUpBeginResponse>(
      `${this.base}/begin`,
      { requestedFor },
      { withCredentials: true },
    );
  }

  verify(
    sessionId: string,
    method: StepUpMethodWire,
    proof: string,
  ): Observable<StepUpVerifyResponse> {
    return this.http.post<StepUpVerifyResponse>(
      `${this.base}/${encodeURIComponent(sessionId)}/verify`,
      { method, proof },
      { withCredentials: true },
    );
  }

  status(sessionId: string): Observable<StepUpStatus> {
    return this.http.get<StepUpStatus>(
      `${this.base}/${encodeURIComponent(sessionId)}/status`,
      { withCredentials: true },
    );
  }

  /** Polling helper — résout dès que `status === 'VERIFIED'` ou timeout. */
  async pollUntilVerified(
    sessionId: string,
    timeoutMs = 5 * 60 * 1000,
    intervalMs = 3000,
  ): Promise<boolean> {
    const start = Date.now();
    while (Date.now() - start < timeoutMs) {
      try {
        const s = await firstValueFrom(this.status(sessionId));
        if (s.status === 'VERIFIED') return true;
        if (s.status === 'FAILED') return false;
      } catch {
        // 404 / network — keep polling within timeout
      }
      await new Promise<void>((r) => setTimeout(r, intervalMs));
    }
    return false;
  }
}
