// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { Injectable } from '@angular/core';
import { Observable, of } from 'rxjs';
import { Review, ReviewStats } from '@shared/models/reputation.models';

@Injectable({ providedIn: 'root' })
export class ReputationService {
  // Mock data until GraphQL mutation/subscription is wired to BFF.
  // Replace with Apollo queries once `reputation.graphql.ts` is plugged in.
  private readonly mockReviews: Review[] = [
    {
      id: 'r1', breederId: '1', clientId: 'c1',
      clientName: 'Fatim Compaoré', rating: 5,
      comment: 'Livraison ponctuelle, poulets en parfaite santé. Je recommande vivement Kassim à tous mes voisins.',
      createdAt: '2026-04-02T10:30:00Z', orderId: 'o-101',
    },
    {
      id: 'r2', breederId: '1', clientId: 'c2',
      clientName: 'Issouf Bandé', rating: 4,
      comment: 'Très bon contact et produit de qualité. Un léger retard sur la livraison mais Kassim a prévenu à temps.',
      createdAt: '2026-03-18T14:12:00Z', orderId: 'o-087',
    },
    {
      id: 'r3', breederId: '1', clientId: 'c3',
      clientName: 'Aïcha Ouédraogo', rating: 5,
      comment: 'Certification halal impeccable, traçabilité complète via QR code. Parfait pour notre restaurant.',
      createdAt: '2026-02-25T09:00:00Z', orderId: 'o-061',
    },
    {
      id: 'r4', breederId: '1', clientId: 'c4',
      clientName: 'Seydou Kaboré', rating: 5,
      comment: 'Des poulets bicyclette de qualité comme au village. Ma famille a adoré.',
      createdAt: '2026-02-08T16:45:00Z',
    },
    {
      id: 'r5', breederId: '1', clientId: 'c5',
      clientName: 'Mariam Sawadogo', rating: 4,
      comment: 'Éleveur sérieux, bonne communication WhatsApp. Prix juste.',
      createdAt: '2026-01-20T11:00:00Z',
    },
  ];

  getReviewsForBreeder(breederId: string, page = 0, size = 10): Observable<{
    content: Review[]; totalElements: number; totalPages: number;
  }> {
    const filtered = this.mockReviews.filter(r => r.breederId === breederId);
    const start = page * size;
    return of({
      content: filtered.slice(start, start + size),
      totalElements: filtered.length,
      totalPages: Math.ceil(filtered.length / size),
    });
  }

  getStats(breederId: string): Observable<ReviewStats> {
    const reviews = this.mockReviews.filter(r => r.breederId === breederId);
    const dist: [number, number, number, number, number] = [0, 0, 0, 0, 0];
    for (const r of reviews) {
      const idx = Math.max(0, Math.min(4, Math.round(r.rating) - 1));
      dist[idx]++;
    }
    const average = reviews.length
      ? reviews.reduce((s, r) => s + r.rating, 0) / reviews.length
      : 0;
    return of({
      breederId,
      average: Math.round(average * 10) / 10,
      total: reviews.length,
      distribution: dist,
    });
  }

  createReview(input: Omit<Review, 'id' | 'createdAt'>): Observable<Review> {
    const review: Review = {
      ...input,
      id: 'r' + Math.random().toString(36).slice(2, 8),
      createdAt: new Date().toISOString(),
    };
    this.mockReviews.unshift(review);
    return of(review);
  }
}
