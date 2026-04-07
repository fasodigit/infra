import { inject } from '@angular/core';
import { CanActivateFn, Router } from '@angular/router';
import { AuthService } from '@services/auth.service';

/**
 * Factory function that returns a CanActivateFn for a specific role.
 * Usage: canActivate: [roleGuard('eleveur')]
 */
export function roleGuard(requiredRole: 'client' | 'eleveur' | 'admin'): CanActivateFn {
  return (route, state) => {
    const auth = inject(AuthService);
    const router = inject(Router);

    if (!auth.isAuthenticated()) {
      return router.createUrlTree(['/login'], {
        queryParams: { returnUrl: state.url },
      });
    }

    const user = auth.currentUser();
    if (user?.role === requiredRole || user?.role === 'admin') {
      return true;
    }

    return router.createUrlTree(['/']);
  };
}
