// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { Routes } from '@angular/router';

export default [
  {
    path: '',
    loadComponent: () => import('./checkout.component').then(m => m.CheckoutComponent),
    title: 'Commande - Poulets BF',
  },
] as Routes;
