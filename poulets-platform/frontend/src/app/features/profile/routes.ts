import { Routes } from '@angular/router';

export default [
  {
    path: '',
    loadComponent: () =>
      import('./profile-view.component').then(m => m.ProfileViewComponent),
    title: 'Mon profil - Poulets Platform',
  },
  {
    path: 'edit',
    loadComponent: () =>
      import('./profile-edit.component').then(m => m.ProfileEditComponent),
    title: 'Modifier profil - Poulets Platform',
  },
  {
    path: 'groupement',
    loadComponent: () =>
      import('./groupement.component').then(m => m.GroupementComponent),
    title: 'Mon groupement - Poulets Platform',
  },
] as Routes;
