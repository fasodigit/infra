// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { HttpClient } from '@angular/common/http';
import { Injectable, inject } from '@angular/core';
import { Observable, of } from 'rxjs';
import { environment } from '../../../../environments/environment';
import type { AuditAction, AuditEntry } from '../models/admin.model';
import { MOCK_AUDIT } from './admin-mocks';

export interface AuditFilters {
  readonly from?: string;
  readonly to?: string;
  readonly actor?: string;
  readonly actions?: readonly AuditAction[];
  readonly ipCidr?: string;
  readonly criticalOnly?: boolean;
}

/**
 * Journal d'audit append-only · rétention 7 ans (Loi 010-2004 BF).
 * Backed by PostgreSQL WAL — immutable.
 */
@Injectable({ providedIn: 'root' })
export class AdminAuditService {
  private readonly http = inject(HttpClient);
  private readonly base = `${environment.bffUrl}/api/admin/audit`;

  query(filters: AuditFilters = {}): Observable<readonly AuditEntry[]> {
    // TODO: return this.http.get<readonly AuditEntry[]>(this.base, { params: ... });
    void filters;
    return of(MOCK_AUDIT);
  }

  getById(id: string): Observable<AuditEntry | undefined> {
    // TODO: return this.http.get<AuditEntry>(`${this.base}/${id}`);
    return of(MOCK_AUDIT.find((a) => a.id === id));
  }

  exportCsv(filters: AuditFilters = {}): Observable<Blob> {
    // TODO: return this.http.get(`${this.base}/export.csv`, { params: ..., responseType: 'blob' });
    void filters;
    return of(new Blob([''], { type: 'text/csv' }));
  }
}
