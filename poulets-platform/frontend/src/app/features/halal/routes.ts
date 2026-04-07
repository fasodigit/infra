import { Routes } from '@angular/router';

export default [
  {
    path: '',
    loadComponent: () =>
      import('./certifications-list.component').then(m => m.CertificationsListComponent),
    title: 'Certification Halal - Poulets Platform',
  },
  {
    path: 'request',
    loadComponent: () =>
      import('./request-certification.component').then(m => m.RequestCertificationComponent),
    title: 'Demande certification - Poulets Platform',
  },
  {
    path: ':id',
    loadComponent: () =>
      import('./certification-detail.component').then(m => m.CertificationDetailComponent),
    title: 'Detail certification - Poulets Platform',
  },
] as Routes;
