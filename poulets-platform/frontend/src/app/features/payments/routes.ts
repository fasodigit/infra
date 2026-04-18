// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { Routes } from '@angular/router';

export default [
  {
    path: 'pay/:txId',
    loadComponent: () =>
      import('./mobile-money-form.component').then(m => m.MobileMoneyFormComponent),
    title: 'Paiement Mobile Money - Poulets BF',
  },
  {
    path: 'pay',
    loadComponent: () =>
      import('./mobile-money-form.component').then(m => m.MobileMoneyFormComponent),
    title: 'Paiement Mobile Money - Poulets BF',
  },
  {
    path: 'escrow/:txId',
    loadComponent: () =>
      import('./escrow.component').then(m => m.EscrowComponent),
    title: 'Paiement sécurisé (séquestre) - Poulets BF',
  },
] as Routes;
