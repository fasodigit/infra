// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { Injectable, signal } from '@angular/core';
import { Observable, delay, of } from 'rxjs';

export interface Organization {
  id: string;
  name: string;
  region: string;
  type: 'COOPERATIVE' | 'GROUPEMENT' | 'ASSOCIATION';
  memberCount: number;
  activeMembers: number;
  totalSales: number;
  avgRating: number;
  contactName: string;
  contactPhone: string;
  contactEmail?: string;
  createdAt: string;
  certifications: string[];
}

@Injectable({ providedIn: 'root' })
export class OrganizationsService {
  private readonly _orgs = signal<Organization[]>(generateMock());

  list(): Observable<Organization[]> { return of(this._orgs()).pipe(delay(120)); }
  get(id: string): Observable<Organization | null> {
    return of(this._orgs().find((o) => o.id === id) ?? null).pipe(delay(100));
  }

  create(input: Omit<Organization, 'id' | 'createdAt' | 'avgRating' | 'totalSales' | 'activeMembers'>): Observable<Organization> {
    const org: Organization = {
      ...input,
      id: 'org-' + Math.random().toString(36).slice(2, 8),
      createdAt: new Date().toISOString(),
      avgRating: 0,
      totalSales: 0,
      activeMembers: 0,
    };
    this._orgs.update((arr) => [org, ...arr]);
    return of(org).pipe(delay(200));
  }
}

function generateMock(): Organization[] {
  return [
    {
      id: 'org-1', name: 'Coopérative Des Éleveurs Du Kadiogo', region: 'Centre', type: 'COOPERATIVE',
      memberCount: 12, activeMembers: 11, totalSales: 1240, avgRating: 4.7,
      contactName: 'Oumar Traoré', contactPhone: '+22670334455', contactEmail: 'cedk@coop.bf',
      createdAt: '2024-06-20', certifications: ['halal', 'vet'],
    },
    {
      id: 'org-2', name: 'Groupement Volailles Hauts-Bassins', region: 'Hauts-Bassins', type: 'GROUPEMENT',
      memberCount: 8, activeMembers: 8, totalSales: 612, avgRating: 4.8,
      contactName: 'Awa Sankara', contactPhone: '+22670223344',
      createdAt: '2023-09-15', certifications: ['halal', 'bio', 'vet'],
    },
    {
      id: 'org-3', name: 'Association Des Producteurs De Pondeuses', region: 'Centre-Ouest', type: 'ASSOCIATION',
      memberCount: 24, activeMembers: 18, totalSales: 2100, avgRating: 4.5,
      contactName: 'Salif Koné', contactPhone: '+22670889900',
      createdAt: '2023-03-10', certifications: ['bio'],
    },
  ];
}
