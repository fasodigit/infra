// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { HttpClient } from '@angular/common/http';
import { Injectable, inject } from '@angular/core';
import { Observable, of } from 'rxjs';
import { environment } from '../../../../environments/environment';

export type OtpMethod = 'email' | 'totp' | 'sms';

export interface OtpIssueResponse {
  readonly otpId: string;
  readonly expiresInSeconds: number;
  readonly resendAvailableInSeconds: number;
}

export interface OtpVerifyResponse {
  readonly verified: boolean;
  readonly attemptsRemaining: number;
}

/**
 * OTP émission / vérification utilisé par les flows sensibles
 * (Break-Glass, Grant-Role, paramètres critiques).
 *
 * Politique : `otp.length`, `otp.lifetime_seconds`, `otp.max_attempts`,
 * `otp.lock_duration_seconds`, `otp.rate_limit_per_user_5min`.
 * KAYA backed (`auth:otp:rl:{userId}`).
 */
@Injectable({ providedIn: 'root' })
export class AdminOtpService {
  private readonly http = inject(HttpClient);
  private readonly base = `${environment.bffUrl}/api/admin/otp`;

  issueOtp(userId: string, method: OtpMethod): Observable<OtpIssueResponse> {
    // TODO: return this.http.post<OtpIssueResponse>(`${this.base}/issue`, { userId, method });
    void userId;
    void method;
    return of({
      otpId: 'pending',
      expiresInSeconds: 300,
      resendAvailableInSeconds: 60,
    });
  }

  verifyOtp(otpId: string, code: string): Observable<OtpVerifyResponse> {
    // TODO: return this.http.post<OtpVerifyResponse>(`${this.base}/verify`, { otpId, code });
    void otpId;
    void code;
    return of({ verified: true, attemptsRemaining: 3 });
  }
}
