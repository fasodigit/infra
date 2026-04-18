import { Routes } from '@angular/router';

export default [
  {
    path: '',
    loadComponent: () =>
      import('./fiches-list.component').then(m => m.FichesListComponent),
    title: 'Fiches sanitaires - Poulets Platform',
  },
  {
    path: 'vaccination/new',
    loadComponent: () =>
      import('./add-vaccination.component').then(m => m.AddVaccinationComponent),
    title: 'Nouvelle vaccination - Poulets Platform',
  },
  {
    path: 'treatment/new',
    loadComponent: () =>
      import('./add-treatment.component').then(m => m.AddTreatmentComponent),
    title: 'Nouveau traitement - Poulets Platform',
  },
  {
    path: ':lotId',
    loadComponent: () =>
      import('./fiche-detail.component').then(m => m.FicheDetailComponent),
    title: 'Fiche sanitaire - Poulets Platform',
  },
  {
    path: ':lotId/record',
    loadComponent: () =>
      import('./components/health-record.component').then(m => m.HealthRecordComponent),
    title: 'Dossier sanitaire - Poulets BF',
  },
] as Routes;
