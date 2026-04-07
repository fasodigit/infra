import { Routes } from '@angular/router';

export default [
  {
    path: '',
    loadComponent: () =>
      import('./admin-dashboard.component').then(m => m.AdminDashboardComponent),
    title: 'Administration - Poulets Platform',
  },
  {
    path: 'users',
    loadComponent: () =>
      import('./admin-users.component').then(m => m.AdminUsersComponent),
    title: 'Utilisateurs - Poulets Platform',
  },
  {
    path: 'transactions',
    loadComponent: () =>
      import('./admin-transactions.component').then(m => m.AdminTransactionsComponent),
    title: 'Transactions - Poulets Platform',
  },
  {
    path: 'stats',
    loadComponent: () =>
      import('./admin-stats.component').then(m => m.AdminStatsComponent),
    title: 'Statistiques - Poulets Platform',
  },
] as Routes;
