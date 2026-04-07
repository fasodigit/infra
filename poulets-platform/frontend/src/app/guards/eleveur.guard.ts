import { inject } from '@angular/core';
import { CanActivateFn, Router } from '@angular/router';
import { AuthService } from '@services/auth.service';

/**
 * Guard that ensures the user has the 'eleveur' role.
 * Redirects to home if not an eleveur.
 */
export const eleveurGuard: CanActivateFn = () => {
  const auth = inject(AuthService);
  const router = inject(Router);

  if (auth.isEleveur()) {
    return true;
  }

  return router.createUrlTree(['/']);
};
