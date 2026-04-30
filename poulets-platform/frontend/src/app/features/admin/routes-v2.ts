// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso
//
// Routes v2 — admin-UI (Claude Design intégré). Mountées sous /admin par l'app-routes.
// L'ancien `routes.ts` (legacy) reste accessible mais sera dépriécié progressivement.

import type { Routes } from '@angular/router';
import { adminGuard } from '@core/guards/admin.guard';

export default [
  {
    path: '',
    loadComponent: () =>
      import('./components-v2/faso-admin-shell.component').then(
        (m) => m.FasoAdminShellComponent,
      ),
    canActivate: [adminGuard],
    children: [
      { path: '', redirectTo: 'dashboard', pathMatch: 'full' },
      {
        path: 'dashboard',
        loadComponent: () =>
          import('./pages-v2/dashboard-v2.page').then((m) => m.DashboardV2Page),
        title: 'Tableau de bord — Admin · FASO',
      },
      {
        path: 'users',
        loadComponent: () =>
          import('./pages-v2/users-list.page').then((m) => m.UsersListPage),
        title: 'Utilisateurs — Admin · FASO',
      },
      {
        path: 'users/:userId',
        loadComponent: () =>
          import('./pages-v2/user-detail.page').then((m) => m.UserDetailPage),
        title: 'Détail utilisateur — Admin · FASO',
      },
      {
        path: 'sessions',
        loadComponent: () =>
          import('./pages-v2/sessions.page').then((m) => m.SessionsPage),
        title: 'Sessions — Admin · FASO',
      },
      {
        path: 'devices',
        loadComponent: () =>
          import('./pages-v2/devices.page').then((m) => m.DevicesPage),
        title: 'Appareils trustés — Admin · FASO',
      },
      {
        path: 'mfa',
        loadComponent: () =>
          import('./pages-v2/mfa.page').then((m) => m.MfaPage),
        title: 'MFA — Admin · FASO',
      },
      {
        path: 'audit',
        loadComponent: () =>
          import('./pages-v2/audit.page').then((m) => m.AuditPage),
        title: "Journal d'audit — Admin · FASO",
      },
      {
        path: 'settings',
        loadComponent: () =>
          import('./pages-v2/settings.page').then((m) => m.SettingsPage),
        title: 'Paramètres — Admin · FASO',
      },
      {
        path: 'break-glass',
        loadComponent: () =>
          import('./pages-v2/break-glass.page').then((m) => m.BreakGlassPage),
        title: 'Break-Glass — Admin · FASO',
      },
      {
        path: 'me/security',
        loadComponent: () =>
          import('./pages-v2/me-security.page').then((m) => m.MeSecurityPage),
        title: 'Mon compte · Sécurité — Admin · FASO',
      },
    ],
  },
] satisfies Routes;
