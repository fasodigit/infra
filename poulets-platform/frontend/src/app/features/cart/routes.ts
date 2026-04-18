// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { Routes } from '@angular/router';

export default [
  {
    path: '',
    loadComponent: () => import('./cart.component').then(m => m.CartComponent),
    title: 'Mon panier - Poulets BF',
  },
] as Routes;
