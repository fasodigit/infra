// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { HttpClient } from '@angular/common/http';
import { Injectable, inject } from '@angular/core';
import { Observable, of } from 'rxjs';
import { environment } from '../../../../environments/environment';
import type { TrustedDevice } from '../models/admin.model';
import { MOCK_DEVICES } from './admin-mocks';

/**
 * Appareils trustés — empreintes UA + IP/24 + Accept-Language stockées
 * dans KAYA avec TTL configurable (`device_trust.ttl_days`).
 */
@Injectable({ providedIn: 'root' })
export class AdminDeviceService {
  private readonly http = inject(HttpClient);
  private readonly base = `${environment.bffUrl}/api/admin/devices`;

  getDevices(): Observable<readonly TrustedDevice[]> {
    // TODO: return this.http.get<readonly TrustedDevice[]>(this.base);
    return of(MOCK_DEVICES);
  }

  trustDevice(id: string): Observable<void> {
    // TODO: return this.http.post<void>(`${this.base}/${id}/trust`, {});
    void id;
    return of(undefined);
  }

  revokeDevice(id: string): Observable<void> {
    // TODO: return this.http.delete<void>(`${this.base}/${id}`);
    void id;
    return of(undefined);
  }
}
