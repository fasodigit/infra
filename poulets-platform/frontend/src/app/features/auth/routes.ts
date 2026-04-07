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
    path: '',
    redirectTo: 'login',
    pathMatch: 'full' as const,
  },
] as Routes;
