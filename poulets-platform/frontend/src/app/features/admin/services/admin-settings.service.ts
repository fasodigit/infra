// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { HttpClient } from '@angular/common/http';
import { Injectable, inject } from '@angular/core';
import { Observable, of } from 'rxjs';
import { environment } from '../../../../environments/environment';
import type { AdminSetting, SettingHistoryEntry } from '../models/admin.model';
import { MOCK_SETTINGS_HISTORY } from './admin-mocks';

/**
 * Configuration Center — versionné en DB, publié sur Redpanda
 * (`admin.settings.updated`), cache BFF 30s avec invalidation auto sur PUT.
 *
 * SUPER-ADMIN uniquement en édition · ADMIN/MANAGER en lecture.
 */
@Injectable({ providedIn: 'root' })
export class AdminSettingsService {
  private readonly http = inject(HttpClient);
  private readonly base = `${environment.bffUrl}/api/admin/settings`;

  getAll(): Observable<readonly AdminSetting[]> {
    // TODO: return this.http.get<readonly AdminSetting[]>(this.base);
    return of([]);
  }

  getByKey<T = unknown>(key: string): Observable<AdminSetting<T> | undefined> {
    // TODO: return this.http.get<AdminSetting<T>>(`${this.base}/${encodeURIComponent(key)}`);
    void key;
    return of(undefined);
  }

  /**
   * Mise à jour optimiste — `version` doit correspondre au snapshot lu
   * (CAS pour éviter les écrasements concurrents). `motif` optionnel mais
   * recommandé pour les paramètres critiques (cf. politique).
   */
  update<T>(
    key: string,
    value: T,
    version: number,
    motif?: string,
  ): Observable<AdminSetting<T>> {
    // TODO: return this.http.put<AdminSetting<T>>(`${this.base}/${encodeURIComponent(key)}`, { value, version, motif });
    void key;
    void value;
    void version;
    void motif;
    return of({} as AdminSetting<T>);
  }

  getHistory(key: string): Observable<readonly SettingHistoryEntry[]> {
    // TODO: return this.http.get<readonly SettingHistoryEntry[]>(`${this.base}/${encodeURIComponent(key)}/history`);
    void key;
    return of(MOCK_SETTINGS_HISTORY);
  }

  /** Restauration d'une version antérieure (publie un nouvel événement). */
  revert(
    key: string,
    targetVersion: number,
    motif: string,
  ): Observable<AdminSetting> {
    // TODO: return this.http.post<AdminSetting>(`${this.base}/${encodeURIComponent(key)}/revert`, { targetVersion, motif });
    void key;
    void targetVersion;
    void motif;
    return of({} as AdminSetting);
  }
}
