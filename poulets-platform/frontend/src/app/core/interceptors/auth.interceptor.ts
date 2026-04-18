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
      // Don't redirect for auth-related API calls (login, register, session check)
      // NOR for Kratos self-service flow calls (settings/login/registration) —
      // ces endpoints renvoient 401 en fonctionnement normal (e.g. flow settings
      // sans session ou AAL1 insuffisant pour certaines actions). Un redirect
      // global sur 401 kratos casserait la page MFA / register / login Angular.
      const isAuthRequest = req.url.includes('/api/auth/');
      const isKratosRequest = req.url.includes('/self-service/')
        || req.url.includes('/sessions/whoami');

      if (error.status === 401 && !isAuthRequest && !isKratosRequest) {
        // Session expired or not authenticated
        const currentUrl = router.url;
        // Only redirect if not already on an auth page
        if (!currentUrl.startsWith('/auth')) {
          router.navigate(['/auth/login'], {
            queryParams: { returnUrl: currentUrl },
          });
        }
      }

      if (error.status === 403) {
        // Authenticated but not authorized
        router.navigate(['/dashboard']);
      }

      return throwError(() => error);
    }),
  );
};
