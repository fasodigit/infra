import { Routes } from '@angular/router';

export default [
  {
    path: '',
    loadComponent: () =>
      import('./components/breeders-map.component').then(m => m.BreedersMapComponent),
    title: 'Carte des éleveurs - Poulets BF',
  },
  {
    path: 'legacy',
    loadComponent: () =>
      import('./map-view.component').then(m => m.MapViewComponent),
    title: 'Carte (legacy) - Poulets Platform',
  },
] as Routes;
