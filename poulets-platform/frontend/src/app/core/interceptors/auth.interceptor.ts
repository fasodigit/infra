import { HttpInterceptorFn, HttpErrorResponse } from '@angular/common/http';
import { inject } from '@angular/core';
import { Router } from '@angular/router';
import { catchError, throwError } from 'rxjs';

/**
 * HTTP interceptor that handles authentication errors.
 * Since auth is cookie-based (httpOnly), we don't attach tokens.
 * We only handle 401/403 responses by redirecting appropriately.
 */
export const authInterceptor: HttpInterceptorFn = (req, next) => {
  const router = inject(Router);

  return next(req).pipe(
    catchError((error: HttpErrorResponse) => {
      if (error.status === 401) {
        // Session expired or not authenticated
        router.navigate(['/auth/login'], {
          queryParams: { returnUrl: router.url },
        });
      }

      if (error.status === 403) {
        // Authenticated but not authorized
        router.navigate(['/dashboard']);
      }

      return throwError(() => error);
    }),
  );
};
