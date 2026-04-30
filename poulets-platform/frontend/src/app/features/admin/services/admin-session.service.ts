// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { HttpClient } from '@angular/common/http';
import { Injectable, inject } from '@angular/core';
import { Observable, of } from 'rxjs';
import { environment } from '../../../../environments/environment';
import type { AdminSession } from '../models/admin.model';
import { MOCK_SESSIONS } from './admin-mocks';

/**
 * Sessions actives — propagation Kratos via Redpanda lors d'une révocation.
 * Endpoints prévus : `/api/admin/sessions`, `DELETE /:id`, `DELETE /` (revoke-all).
 */
@Injectable({ providedIn: 'root' })
export class AdminSessionService {
  private readonly http = inject(HttpClient);
  private readonly base = `${environment.bffUrl}/api/admin/sessions`;

  getSessions(): Observable<readonly AdminSession[]> {
    // TODO: return this.http.get<readonly AdminSession[]>(this.base);
    return of(MOCK_SESSIONS);
  }

  revokeSession(id: string): Observable<void> {
    // TODO: return this.http.delete<void>(`${this.base}/${id}`);
    void id;
    return of(undefined);
  }

  revokeAll(): Observable<void> {
    // TODO: return this.http.delete<void>(this.base);
    return of(undefined);
  }
}
