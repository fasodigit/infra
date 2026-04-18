// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { Injectable, inject, signal } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Observable, of } from 'rxjs';
import { catchError } from 'rxjs/operators';

export interface SmsOtpSendRequest {
  phone: string;
}

export interface SmsOtpSendResponse {
  sent: boolean;
  expiresAt?: string;
  message?: string;
}

const STORAGE_KEY = 'faso_otp_channel';

/**
 * SMS OTP fallback service.
 *
 * Lets users opt to receive the one-time password by SMS instead of e-mail —
 * useful on low-bandwidth / rural connectivity where e-mail may lag.
 *
 * The choice is persisted in `localStorage` under `faso_otp_channel`
 * (`'sms' | 'email'`). The server-side endpoint lives at
 * `POST /api/auth/sms-otp` (BFF).
 */
@Injectable({ providedIn: 'root' })
export class SmsOtpService {
  private readonly http = inject(HttpClient);
  private readonly endpoint = '/api/auth/sms-otp';

  /** Current OTP channel preference ('sms' | 'email'). */
  readonly channel = signal<'sms' | 'email'>(this.readPersisted());

  setChannel(value: 'sms' | 'email'): void {
    this.channel.set(value);
    try {
      localStorage.setItem(STORAGE_KEY, value);
    } catch {
      // Storage unavailable
    }
  }

  send(req: SmsOtpSendRequest): Observable<SmsOtpSendResponse> {
    return this.http.post<SmsOtpSendResponse>(this.endpoint, req).pipe(
      catchError(() => {
        // Keep UX responsive when offline — mark pending.
        return of<SmsOtpSendResponse>({
          sent: false,
          message: 'Envoi du SMS impossible pour le moment',
        });
      }),
    );
  }

  private readPersisted(): 'sms' | 'email' {
    try {
      const v = localStorage.getItem(STORAGE_KEY);
      return v === 'sms' ? 'sms' : 'email';
    } catch {
      return 'email';
    }
  }
}
