import { Routes } from '@angular/router';

export default [
  {
    path: '',
    loadComponent: () =>
      import('./dashboard-redirect.component').then(m => m.DashboardRedirectComponent),
    title: 'Tableau de bord - Poulets Platform',
  },
  {
    path: 'eleveur',
    loadComponent: () =>
      import('./eleveur-dashboard.component').then(m => m.EleveurDashboardComponent),
    title: 'Tableau de bord Eleveur - Poulets Platform',
  },
  {
    path: 'client',
    loadComponent: () =>
      import('./client-dashboard.component').then(m => m.ClientDashboardComponent),
    title: 'Tableau de bord Client - Poulets Platform',
  },
  {
    path: 'producteur',
    loadComponent: () =>
      import('./producteur-dashboard.component').then(m => m.ProducteurDashboardComponent),
    title: 'Tableau de bord Producteur - Poulets Platform',
  },
  {
    path: 'admin',
    loadComponent: () =>
      import('./admin-dashboard.component').then(m => m.AdminDashboardComponent),
    title: 'Administration - Poulets Platform',
  },
  // Analytique vendeur (F10) — KPI + graphiques (stub MVP)
  {
    path: 'analytics',
    loadComponent: () =>
      import('./analytics.component').then(m => m.AnalyticsComponent),
    title: 'Analytique vendeur - Poulets Platform',
  },
] as Routes;
