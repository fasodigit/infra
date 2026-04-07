import { Routes } from '@angular/router';

export default [
  {
    path: '',
    loadComponent: () =>
      import('./components/marketplace-home.component').then(m => m.MarketplaceHomeComponent),
    title: 'Marketplace - Poulets Platform',
  },
  {
    path: 'annonces',
    loadComponent: () =>
      import('./components/annonces-list.component').then(m => m.AnnoncesListComponent),
    title: 'Annonces - Poulets Platform',
  },
  {
    path: 'annonces/new',
    loadComponent: () =>
      import('./components/create-annonce.component').then(m => m.CreateAnnonceComponent),
    title: 'Nouvelle Annonce - Poulets Platform',
  },
  {
    path: 'annonces/:id',
    loadComponent: () =>
      import('./components/annonce-detail.component').then(m => m.AnnonceDetailComponent),
    title: 'Annonce - Poulets Platform',
  },
  {
    path: 'besoins',
    loadComponent: () =>
      import('./components/besoins-list.component').then(m => m.BesoinsListComponent),
    title: 'Besoins - Poulets Platform',
  },
  {
    path: 'besoins/new',
    loadComponent: () =>
      import('./components/create-besoin.component').then(m => m.CreateBesoinComponent),
    title: 'Nouveau Besoin - Poulets Platform',
  },
  {
    path: 'besoins/:id',
    loadComponent: () =>
      import('./components/besoin-detail.component').then(m => m.BesoinDetailComponent),
    title: 'Besoin - Poulets Platform',
  },
  {
    path: 'matching',
    loadComponent: () =>
      import('./components/matching.component').then(m => m.MatchingComponent),
    title: 'Matching - Poulets Platform',
  },
] as Routes;
