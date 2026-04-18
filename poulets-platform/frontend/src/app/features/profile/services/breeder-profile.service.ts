// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { Injectable } from '@angular/core';
import { Observable, of } from 'rxjs';
import { BreederProfile } from '@shared/models/reputation.models';

@Injectable({ providedIn: 'root' })
export class BreederProfileService {
  private readonly mock: Record<string, BreederProfile> = {
    '1': {
      id: '1',
      name: 'Ouédraogo',
      prenom: 'Kassim',
      avatar: null,
      coverPhoto: null,
      region: 'Centre',
      city: 'Ouagadougou',
      latitude: 12.3714,
      longitude: -1.5197,
      distanceKm: 8,
      bio: 'Éleveur depuis 2014, je mets un point d\'honneur à produire des poulets bicyclette de qualité, nourris aux grains locaux. Ma ferme familiale est certifiée halal et suivie par un vétérinaire agréé.',
      specialties: ['Poulet bicyclette', 'Race locale', 'Halal'],
      halalCertified: true,
      veterinaryVerified: true,
      bioCertified: false,
      memberSince: '2024-02-01',
      responseTimeHours: 2,
      totalSales: 348,
      gallery: [],
      phone: '+22670112233',
      whatsapp: '+22670112233',
    },
    '2': {
      id: '2',
      name: 'Sankara',
      prenom: 'Awa',
      avatar: null,
      coverPhoto: null,
      region: 'Hauts-Bassins',
      city: 'Bobo-Dioulasso',
      latitude: 11.1779,
      longitude: -4.2979,
      distanceKm: 360,
      bio: 'Spécialiste des pondeuses bio depuis 10 ans. Mes poules vivent en plein air et reçoivent une alimentation naturelle sans OGM. Livraison hebdomadaire sur Bobo et environs.',
      specialties: ['Pondeuses', 'Bio', 'Œufs fermiers'],
      halalCertified: true,
      veterinaryVerified: true,
      bioCertified: true,
      memberSince: '2023-09-15',
      responseTimeHours: 4,
      totalSales: 612,
      gallery: [],
    },
    '3': {
      id: '3',
      name: 'Traoré',
      prenom: 'Oumar',
      avatar: null,
      coverPhoto: null,
      region: 'Centre-Ouest',
      city: 'Koudougou',
      latitude: 12.2530,
      longitude: -2.3622,
      distanceKm: 98,
      bio: 'Coopérative de 12 éleveurs locaux. Nous mutualisons nos lots pour livrer les restaurants, hôtels et grandes familles. Traçabilité complète par QR code.',
      specialties: ['Coopérative', 'Poulet de chair', 'Gros volumes'],
      halalCertified: true,
      veterinaryVerified: true,
      bioCertified: false,
      memberSince: '2024-06-20',
      responseTimeHours: 3,
      totalSales: 1240,
      gallery: [],
    },
  };

  getById(id: string): Observable<BreederProfile | null> {
    return of(this.mock[id] ?? null);
  }

  list(): Observable<BreederProfile[]> {
    return of(Object.values(this.mock));
  }
}
