// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { Routes } from '@angular/router';

export default [
  {
    path: '',
    loadComponent: () =>
      import('./components/organizations-list.component').then((m) => m.OrganizationsListComponent),
    title: 'Organisations - Poulets BF',
  },
] as Routes;
