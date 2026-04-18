// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION

import { Injectable, inject } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { BehaviorSubject, Observable, firstValueFrom, of } from 'rxjs';
import { catchError, map } from 'rxjs/operators';
import { environment } from '@env/environment';

/**
 * Réponse du BFF `/api/flags` :
 *   { "flags": { "poulets.new-checkout": true, "etat-civil.pdf-v2": false, ... } }
 */
export interface FlagsPayload {
  flags: Record<string, boolean>;
  env: string;
  fetchedAt: string;
}

/**
 * FeatureFlagsService — singleton Angular. Bootstrap via APP_INITIALIZER
 * (`featureFlagsInitializer`) pour que les flags soient disponibles avant
 * le premier rendu composants.
 *
 * Source : BFF `/api/flags` (qui lui-même cache via KAYA / backend Java).
 * Le frontend ne parle jamais directement à GrowthBook (ni à KAYA) pour
 * éviter toute exfiltration de clef SDK côté navigateur.
 */
@Injectable({ providedIn: 'root' })
export class FeatureFlagsService {
  private readonly http = inject(HttpClient);
  private readonly _flags$ = new BehaviorSubject<Record<string, boolean>>({});

  /** Observable public — à consommer via pipe `| async` ou directive `*fasoFeature`. */
  readonly flags$: Observable<Record<string, boolean>> = this._flags$.asObservable();

  /** Valeur synchrone courante d'un flag. Retourne `false` si inconnu. */
  isOn(key: string): boolean {
    return this._flags$.value[key] === true;
  }

  /** Bootstrap : appelé une fois au démarrage via APP_INITIALIZER. */
  async load(): Promise<void> {
    const url = `${environment.bffUrl ?? '/api'}/flags`;
    try {
      const payload = await firstValueFrom(
        this.http.get<FlagsPayload>(url).pipe(
          catchError(() => of<FlagsPayload>({ flags: {}, env: 'unknown', fetchedAt: new Date().toISOString() })),
        ),
      );
      this._flags$.next(payload.flags ?? {});
    } catch {
      // Fail-open : pas de flags → tout défaut à false.
      this._flags$.next({});
    }
  }

  /** Force un refresh (ex. après login pour recharger les flags user-targeted). */
  refresh(): Observable<Record<string, boolean>> {
    const url = `${environment.bffUrl ?? '/api'}/flags`;
    return this.http.get<FlagsPayload>(url).pipe(
      map((p) => p.flags ?? {}),
      catchError(() => of({})),
    );
  }
}

/**
 * Factory APP_INITIALIZER. Usage dans `app.config.ts` :
 *
 * ```ts
 * provideAppInitializer(() => inject(FeatureFlagsService).load()),
 * ```
 *
 * Ou (legacy DI factory API) :
 * ```ts
 * { provide: APP_INITIALIZER, useFactory: featureFlagsInitializer, deps: [FeatureFlagsService], multi: true }
 * ```
 */
export function featureFlagsInitializer(svc: FeatureFlagsService): () => Promise<void> {
  return () => svc.load();
}
