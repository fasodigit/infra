import { Routes } from '@angular/router';

export default [
  {
    path: '',
    loadComponent: () =>
      import('./components/contracts-list.component').then(m => m.ContractsListComponent),
    title: 'Contrats - Poulets Platform',
  },
  {
    path: 'new',
    loadComponent: () =>
      import('./components/create-contract.component').then(m => m.CreateContractComponent),
    title: 'Nouveau Contrat - Poulets Platform',
  },
  {
    path: ':id',
    loadComponent: () =>
      import('./components/contract-detail.component').then(m => m.ContractDetailComponent),
    title: 'Contrat - Poulets Platform',
  },
] as Routes;
