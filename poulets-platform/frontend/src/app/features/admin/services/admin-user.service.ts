// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { HttpClient } from '@angular/common/http';
import { Injectable, inject } from '@angular/core';
import { Observable, of } from 'rxjs';
import { environment } from '../../../../environments/environment';
import type { AdminLevel, AdminUser } from '../models/admin.model';
import { MOCK_USERS } from './admin-mocks';

/**
 * Wrapper HTTP pour la gestion des comptes administrateurs.
 *
 * Pour l'instant, retourne des mocks. Les méthodes utiliseront `HttpClient`
 * vers le BFF (`environment.bffUrl + /api/admin/users`) une fois les
 * endpoints câblés côté backend.
 */
@Injectable({ providedIn: 'root' })
export class AdminUserService {
  // TODO: décommenter et utiliser quand les endpoints BFF sont prêts.
  private readonly http = inject(HttpClient);
  private readonly base = `${environment.bffUrl}/api/admin/users`;

  /** Liste paginable de l'ensemble des comptes administrateurs. */
  getUsers(): Observable<readonly AdminUser[]> {
    // TODO: return this.http.get<readonly AdminUser[]>(this.base);
    return of(MOCK_USERS);
  }

  /** Détail d'un compte (incluant MFA, devices count, etc.). */
  getUser(id: string): Observable<AdminUser | undefined> {
    // TODO: return this.http.get<AdminUser>(`${this.base}/${id}`);
    return of(MOCK_USERS.find((u) => u.id === id));
  }

  /** Invitation d'un nouvel administrateur (envoie e-mail magic-link). */
  inviteAdmin(email: string, role: AdminLevel): Observable<{ id: string }> {
    // TODO: return this.http.post<{ id: string }>(`${this.base}/invite`, { email, role });
    void email;
    void role;
    return of({ id: 'pending' });
  }

  /** Suspend un compte (perd toutes ses sessions actives). */
  suspendUser(id: string): Observable<void> {
    // TODO: return this.http.post<void>(`${this.base}/${id}/suspend`, {});
    void id;
    return of(undefined);
  }
}
