// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { Injectable, computed, signal } from '@angular/core';
import { Observable, delay, of } from 'rxjs';
import { ModerationItem, ModerationStatus } from '../models';

const LOCK_DURATION_MIN = 15;

@Injectable({ providedIn: 'root' })
export class ModerationService {
  private readonly _items = signal<ModerationItem[]>(generateMock());
  /** ID de l'admin courant — en prod on l'obtient de AuthService. */
  readonly currentAdminId = signal('admin-current');
  readonly currentAdminName = signal('Admin FASO');

  readonly items = this._items.asReadonly();
  readonly pendingCount = computed(() => this._items().filter((i) => i.status === 'pending').length);
  readonly inReviewCount = computed(() => this._items().filter((i) => i.status === 'in_review').length);

  list(status?: ModerationStatus): Observable<ModerationItem[]> {
    const arr = status ? this._items().filter((i) => i.status === status) : this._items();
    return of(arr).pipe(delay(120));
  }

  get(id: string): Observable<ModerationItem | null> {
    return of(this._items().find((i) => i.id === id) ?? null).pipe(delay(100));
  }

  /** Pose un lock pessimiste de 15 min pour l'admin courant. */
  lock(id: string): Observable<ModerationItem | null> {
    const until = new Date(Date.now() + LOCK_DURATION_MIN * 60000).toISOString();
    this._items.update((arr) => arr.map((i) => i.id === id
      ? { ...i, status: 'in_review', lockedBy: this.currentAdminName(), lockedUntil: until,
          history: [...(i.history ?? []), { at: new Date().toISOString(), actorName: this.currentAdminName(), action: 'lock' }] }
      : i,
    ));
    return this.get(id);
  }

  unlock(id: string): Observable<ModerationItem | null> {
    this._items.update((arr) => arr.map((i) => i.id === id
      ? { ...i, lockedBy: undefined, lockedUntil: undefined,
          history: [...(i.history ?? []), { at: new Date().toISOString(), actorName: this.currentAdminName(), action: 'unlock' }] }
      : i,
    ));
    return this.get(id);
  }

  approve(id: string, comment?: string): Observable<ModerationItem | null> {
    this._items.update((arr) => arr.map((i) => i.id === id
      ? { ...i, status: 'approved',
          history: [...(i.history ?? []), { at: new Date().toISOString(), actorName: this.currentAdminName(), action: 'approve', comment }] }
      : i,
    ));
    return this.get(id);
  }

  reject(id: string, comment: string): Observable<ModerationItem | null> {
    this._items.update((arr) => arr.map((i) => i.id === id
      ? { ...i, status: 'rejected',
          history: [...(i.history ?? []), { at: new Date().toISOString(), actorName: this.currentAdminName(), action: 'reject', comment }] }
      : i,
    ));
    return this.get(id);
  }

  escalate(id: string, comment?: string): Observable<ModerationItem | null> {
    this._items.update((arr) => arr.map((i) => i.id === id
      ? { ...i, status: 'escalated', requiresFourEyes: true,
          history: [...(i.history ?? []), { at: new Date().toISOString(), actorName: this.currentAdminName(), action: 'escalate', comment }] }
      : i,
    ));
    return this.get(id);
  }

  fourEyesApprove(id: string): Observable<ModerationItem | null> {
    this._items.update((arr) => arr.map((i) => {
      if (i.id !== id) return i;
      const approvals = [...(i.fourEyesApprovals ?? []), {
        adminId: this.currentAdminId(),
        adminName: this.currentAdminName(),
        at: new Date().toISOString(),
      }];
      // Reject duplicate by same admin
      const uniq = Array.from(new Map(approvals.map((a) => [a.adminId, a])).values());
      return { ...i, fourEyesApprovals: uniq,
        history: [...(i.history ?? []), { at: new Date().toISOString(), actorName: this.currentAdminName(), action: 'four-eyes-approve' }] };
    }));
    return this.get(id);
  }
}

function generateMock(): ModerationItem[] {
  const now = Date.now();
  return [
    {
      id: 'm-001',
      type: 'HALAL_CERT_REVIEW',
      priority: 'P0',
      status: 'pending',
      title: 'Certification halal · lot L-2026-041',
      summary: 'Demande de certification halal par Kassim Ouédraogo. Étape 3 en attente de validation.',
      authorId: '1', authorName: 'Kassim Ouédraogo',
      region: 'Centre',
      createdAt: new Date(now - 3 * 3600000).toISOString(),
      slaRemainingMin: 24 * 60,
      attachments: [
        { id: 'a1', name: 'fiche-abattoir.pdf', mime: 'application/pdf', url: '#' },
        { id: 'a2', name: 'photo-installation.jpg', mime: 'image/jpeg', url: 'assets/img/placeholder-poulet.svg' },
      ],
      history: [{ at: new Date(now - 3 * 3600000).toISOString(), actorName: 'Kassim Ouédraogo', action: 'create' }],
      requiresFourEyes: true,
    },
    {
      id: 'm-002',
      type: 'ANNONCE_FLAGGED',
      priority: 'P1',
      status: 'pending',
      title: 'Annonce signalée : "Poulets très gros"',
      summary: 'Annonce signalée 2 fois pour "description trompeuse" (taille annoncée vs photos).',
      authorId: '7', authorName: 'Inconnu (signalé)',
      region: 'Hauts-Bassins',
      createdAt: new Date(now - 8 * 3600000).toISOString(),
      slaRemainingMin: 12 * 60,
      attachments: [
        { id: 'a3', name: 'capture-annonce.jpg', mime: 'image/jpeg', url: 'assets/img/placeholder-poulet.svg' },
      ],
      history: [{ at: new Date(now - 8 * 3600000).toISOString(), actorName: 'Système', action: 'create' }],
    },
    {
      id: 'm-003',
      type: 'USER_REPORT',
      priority: 'P2',
      status: 'pending',
      title: 'Utilisateur signalé : comportement suspect',
      summary: 'Client signalé pour annulations répétées. 4 annulations en 7 jours.',
      authorId: '12', authorName: 'Signalement automatique',
      region: 'Centre',
      createdAt: new Date(now - 20 * 3600000).toISOString(),
      slaRemainingMin: 48 * 60,
    },
    {
      id: 'm-004',
      type: 'ANNONCE_NEW',
      priority: 'P2',
      status: 'in_review',
      title: 'Nouvelle annonce : "50 pondeuses bio"',
      summary: 'Annonce soumise par Awa Sankara. Vérif halal + fiche vet à confirmer.',
      authorId: '2', authorName: 'Awa Sankara',
      region: 'Hauts-Bassins',
      createdAt: new Date(now - 3 * 3600000).toISOString(),
      slaRemainingMin: 6 * 60,
      lockedBy: 'Admin FASO',
      lockedUntil: new Date(now + 12 * 60000).toISOString(),
    },
    {
      id: 'm-005',
      type: 'REVIEW_FLAGGED',
      priority: 'P2',
      status: 'approved',
      title: 'Avis 1★ signalé comme injuste',
      summary: 'Éleveur conteste. Preuve de livraison correcte fournie.',
      authorId: '3', authorName: 'Oumar Traoré',
      region: 'Centre-Ouest',
      createdAt: new Date(now - 48 * 3600000).toISOString(),
      slaRemainingMin: 0,
    },
  ];
}
