// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { HttpInterceptorFn, HttpErrorResponse, HttpRequest } from '@angular/common/http';
import { inject } from '@angular/core';
import { Observable, throwError } from 'rxjs';
import { catchError, filter, switchMap, take } from 'rxjs/operators';

import {
  StepUpRequiredPayload,
  StepUpService,
} from '../../features/admin/services/step-up.service';

/**
 * HTTP interceptor (Phase 4.b.7) — détecte les 401 de step-up et délègue au
 * composant `<faso-step-up-guard>` qui ouvre le modal Material. Au succès,
 * la requête originale est rejouée avec
 * `Authorization: Bearer <stepUpToken>`.
 *
 * Doit être enregistré APRÈS `authInterceptor` (qui gère 401 sans body
 * `step_up_required`).
 */
export const stepUpInterceptor: HttpInterceptorFn = (req, next) => {
  const stepUp = inject(StepUpService);

  return next(req).pipe(
    catchError((error: HttpErrorResponse) => {
      if (error.status !== 401) return throwError(() => error);

      const body = error.error;
      if (!StepUpService.isStepUpRequired(body)) {
        return throwError(() => error);
      }

      const payload = body as StepUpRequiredPayload;
      const sessionId = payload.step_up_session_id;
      stepUp.registerPending(payload, req.urlWithParams);

      return waitForToken(stepUp, sessionId).pipe(
        switchMap((token: string | null) => {
          if (!token) return throwError(() => error);
          const retried: HttpRequest<unknown> = req.clone({
            setHeaders: { Authorization: `Bearer ${token}` },
          });
          return next(retried);
        }),
      );
    }),
  );
};

function waitForToken(
  stepUp: StepUpService,
  sessionId: string,
): Observable<string | null> {
  return stepUp.tokenStream.asObservable().pipe(
    filter((event) => event.sessionId === sessionId),
    take(1),
    switchMap((event) =>
      new Observable<string | null>((sub) => {
        sub.next(event.token);
        sub.complete();
      }),
    ),
  );
}
