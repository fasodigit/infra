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
  {
    path: 'eleveur/:id',
    loadComponent: () =>
      import('./components/breeder-profile.component').then(m => m.BreederProfileComponent),
    title: 'Profil éleveur - Poulets Platform',
  },
  {
    path: 'mfa',
    loadComponent: () =>
      import('./components/mfa-settings.component').then(m => m.MfaSettingsComponent),
    title: 'Sécurité - Poulets BF',
  },
  {
    path: 'security',
    loadComponent: () =>
      import('./components/security-sessions.component').then(m => m.SecuritySessionsComponent),
    title: 'Sessions actives - Poulets BF',
  },
  // KYC renforcé (F6) — vérification d'identité CNIB + selfie
  {
    path: 'kyc',
    loadComponent: () =>
      import('./components/kyc.component').then(m => m.KycComponent),
    title: 'Vérification d\'identité - Poulets BF',
  },
  // Notifications push (F8) — distinct de /notifications (inbox)
  {
    path: 'notifications-push',
    loadComponent: () =>
      import('./components/notifications-push.component').then(m => m.NotificationsPushComponent),
    title: 'Notifications push - Poulets BF',
  },
] as Routes;
