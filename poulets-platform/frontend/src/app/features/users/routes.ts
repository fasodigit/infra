// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { Routes } from '@angular/router';

export default [
  {
    path: '',
    loadComponent: () =>
      import('./components/users-list.component').then((m) => m.UsersListComponent),
    title: 'Utilisateurs - Poulets BF',
  },
  {
    path: 'create',
    loadComponent: () =>
      import('./components/user-create-wizard.component').then((m) => m.UserCreateWizardComponent),
    title: 'Créer un utilisateur - Poulets BF',
  },
  {
    path: ':id',
    loadComponent: () =>
      import('./components/user-detail.component').then((m) => m.UserDetailComponent),
    title: 'Détails utilisateur - Poulets BF',
  },
] as Routes;
