// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { inject } from '@angular/core';
import { CanActivateFn, Router } from '@angular/router';
import { AuthService } from '@core/services/auth.service';

/**
 * Guard : ADMIN uniquement. Sinon redirige vers /403 (error-page).
 */
export const adminGuard: CanActivateFn = () => {
  const auth = inject(AuthService);
  const router = inject(Router);

  if (!auth.isLoggedIn()) {
    router.navigate(['/auth/login']);
    return false;
  }

  // Poulets AuthService expose userRole() computed et isAdmin() helper.
  // Fallback défensif si l'une des deux manque.
  const isAdmin =
    (typeof (auth as any).isAdmin === 'function' && (auth as any).isAdmin())
    || ((auth as any).userRole?.() === 'ADMIN');

  if (!isAdmin) {
    router.navigate(['/404']);
    return false;
  }
  return true;
};
