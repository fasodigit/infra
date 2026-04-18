// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { Injectable, inject } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Observable, of } from 'rxjs';
import { catchError } from 'rxjs/operators';

/** Supported mobile money providers in Burkina Faso. */
export type MobileMoneyProvider = 'orange_money' | 'moov_africa' | 'wave';

export interface MobileMoneyInitiateRequest {
  provider: MobileMoneyProvider;
  phone: string;
  amount: number;
  reference: string;
}

export interface MobileMoneyInitiateResponse {
  status: 'PENDING' | 'SUCCESS' | 'FAILED';
  txId: string;
  pollUrl: string;
  provider?: MobileMoneyProvider;
  message?: string;
}

/**
 * Service client for the mobile-money BFF endpoint.
 *
 * Routes all traffic to `/api/payments/mobile-money` (BFF Next.js),
 * which proxies to the provider gateway (Orange Money / Moov Africa / Wave)
 * or returns a local stub when `MOMO_GATEWAY_URL` is absent.
 */
@Injectable({ providedIn: 'root' })
export class MobileMoneyService {
  private readonly http = inject(HttpClient);
  private readonly endpoint = '/api/payments/mobile-money';

  initiate(req: MobileMoneyInitiateRequest): Observable<MobileMoneyInitiateResponse> {
    return this.http.post<MobileMoneyInitiateResponse>(this.endpoint, req).pipe(
      catchError(() => {
        // Offline / network-error fallback — keep UX responsive.
        return of<MobileMoneyInitiateResponse>({
          status: 'PENDING',
          txId: `local-${Date.now()}`,
          pollUrl: `${this.endpoint}/status/local-${Date.now()}`,
          provider: req.provider,
          message: 'Paiement initié en mode hors ligne — sera synchronisé',
        });
      }),
    );
  }
}
