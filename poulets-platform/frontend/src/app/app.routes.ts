import { Routes } from '@angular/router';
import { authGuard } from './guards/auth.guard';
import { eleveurGuard } from './guards/eleveur.guard';

export const routes: Routes = [
  // Public routes
  {
    path: '',
    loadComponent: () =>
      import('./components/home/home.component').then((m) => m.HomeComponent),
    title: 'Accueil - Poulets Platform',
  },
  {
    path: 'login',
    loadComponent: () =>
      import('./components/login/login.component').then((m) => m.LoginComponent),
    title: 'Connexion - Poulets Platform',
  },
  {
    path: 'register',
    loadComponent: () =>
      import('./components/register/register.component').then((m) => m.RegisterComponent),
    title: 'Inscription - Poulets Platform',
  },
  {
    path: 'client/catalogue',
    loadComponent: () =>
      import('./components/catalogue/catalogue.component').then((m) => m.CatalogueComponent),
    title: 'Catalogue - Poulets Platform',
  },

  // Authenticated client routes
  {
    path: 'client/panier',
    loadComponent: () =>
      import('./components/panier/panier.component').then((m) => m.PanierComponent),
    canActivate: [authGuard],
    title: 'Panier - Poulets Platform',
  },
  {
    path: 'client/commandes',
    loadComponent: () =>
      import('./components/client-commandes/client-commandes.component').then(
        (m) => m.ClientCommandesComponent,
      ),
    canActivate: [authGuard],
    title: 'Mes Commandes - Poulets Platform',
  },

  // Eleveur (farmer) routes
  {
    path: 'eleveur/dashboard',
    loadComponent: () =>
      import('./components/eleveur-dashboard/eleveur-dashboard.component').then(
        (m) => m.EleveurDashboardComponent,
      ),
    canActivate: [authGuard, eleveurGuard],
    title: 'Tableau de bord - Poulets Platform',
  },
  {
    path: 'eleveur/poulets',
    loadComponent: () =>
      import('./components/eleveur-poulets/eleveur-poulets.component').then(
        (m) => m.EleveurPouletsComponent,
      ),
    canActivate: [authGuard, eleveurGuard],
    title: 'Mes Poulets - Poulets Platform',
  },

  // Wildcard redirect
  {
    path: '**',
    redirectTo: '',
  },
];
