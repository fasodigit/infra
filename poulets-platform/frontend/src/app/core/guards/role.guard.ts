import { inject } from '@angular/core';
import { CanActivateFn, Router } from '@angular/router';
import { AuthService } from '@core/services/auth.service';
import { Role } from '@app/shared/models/user.model';

/**
 * Factory function that returns a CanActivateFn for one or more required roles.
 * Admin always passes.
 *
 * Usage: canActivate: [roleGuard('eleveur')]
 *        canActivate: [roleGuard('eleveur', 'client')]
 */
export function roleGuard(...requiredRoles: Role[]): CanActivateFn {
  return (route, state) => {
    const auth = inject(AuthService);
    const router = inject(Router);

    if (!auth.isLoggedIn()) {
      return router.createUrlTree(['/auth/login'], {
        queryParams: { returnUrl: state.url },
      });
    }

    // Admin has access to everything
    if (auth.isAdmin()) {
      return true;
    }

    if (auth.hasAnyRole(...requiredRoles)) {
      return true;
    }

    // Not authorized for this route
    return router.createUrlTree(['/dashboard']);
  };
}
