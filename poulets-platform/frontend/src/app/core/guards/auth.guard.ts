import { inject } from '@angular/core';
import { CanActivateFn, Router } from '@angular/router';
import { AuthService } from '@core/services/auth.service';

/**
 * Guard that ensures the user is authenticated.
 * Waits for the session check to complete before deciding.
 * Redirects to /auth/login if not authenticated.
 */
export const authGuard: CanActivateFn = async (route, state) => {
  const auth = inject(AuthService);
  const router = inject(Router);

  // Wait for session initialization if not yet done
  if (!auth.initialized()) {
    await new Promise<void>((resolve) => {
      const check = () => {
        if (auth.initialized()) {
          resolve();
        } else {
          setTimeout(check, 50);
        }
      };
      check();
    });
  }

  if (auth.isLoggedIn()) {
    return true;
  }

  return router.createUrlTree(['/auth/login'], {
    queryParams: { returnUrl: state.url },
  });
};
