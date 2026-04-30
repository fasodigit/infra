import { Routes } from '@angular/router';

export default [
  {
    path: 'login',
    loadComponent: () =>
      import('./login/login.component').then(m => m.LoginComponent),
    title: 'Connexion - Poulets Platform',
  },
  {
    path: 'register',
    loadComponent: () =>
      import('./register/register.component').then(m => m.RegisterComponent),
    title: 'Inscription - Poulets Platform',
  },
  {
    path: 'forgot-password',
    loadComponent: () =>
      import('./forgot-password/forgot-password.component').then(m => m.ForgotPasswordComponent),
    title: 'Mot de passe oublie - Poulets Platform',
  },
  {
    path: 'mfa',
    loadComponent: () =>
      import('./components/mfa-challenge.component').then(m => m.MfaChallengeComponent),
    title: 'Vérification 2 étapes - Poulets BF',
  },
  {
    path: 'recovery',
    loadComponent: () =>
      import('./pages/recovery.page').then(m => m.RecoveryPage),
    title: 'Récupération de compte - Poulets BF',
  },
  {
    path: 'admin-onboard',
    loadComponent: () =>
      import('./pages/admin-onboard.page').then(m => m.AdminOnboardPage),
    title: 'Activation administrateur - FASO DIGITALISATION',
  },
  {
    path: '',
    redirectTo: 'login',
    pathMatch: 'full' as const,
  },
] as Routes;
