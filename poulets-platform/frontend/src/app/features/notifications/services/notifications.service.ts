// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { Injectable, computed, signal } from '@angular/core';
import { Observable, delay, of } from 'rxjs';

export type NotificationType = 'ORDER_UPDATE' | 'MESSAGE' | 'REVIEW' | 'CERTIFICATION' | 'SYSTEM' | 'MFA_REMINDER';

export interface AppNotification {
  id: string;
  type: NotificationType;
  title: string;
  body: string;
  createdAt: string;
  read: boolean;
  link?: string;
  actorName?: string;
}

@Injectable({ providedIn: 'root' })
export class NotificationsService {
  private readonly _items = signal<AppNotification[]>(generateMock());

  readonly items = this._items.asReadonly();
  readonly unreadCount = computed(() => this._items().filter((n) => !n.read).length);

  list(page = 0, size = 30, type?: NotificationType, unreadOnly = false): Observable<AppNotification[]> {
    const filtered = this._items().filter((n) => {
      if (type && n.type !== type) return false;
      if (unreadOnly && n.read) return false;
      return true;
    });
    return of(filtered.slice(page * size, page * size + size)).pipe(delay(120));
  }

  markRead(id: string): void {
    this._items.update((arr) => arr.map((n) => n.id === id ? { ...n, read: true } : n));
  }

  markAllRead(): void {
    this._items.update((arr) => arr.map((n) => ({ ...n, read: true })));
  }

  delete(id: string): void {
    this._items.update((arr) => arr.filter((n) => n.id !== id));
  }

  deleteAll(): void {
    this._items.set([]);
  }

  /** Appelé par SSE en prod. Stub ici pour le dev. */
  push(n: Omit<AppNotification, 'id' | 'createdAt' | 'read'>): void {
    this._items.update((arr) => [{
      ...n,
      id: 'n-' + Math.random().toString(36).slice(2, 8),
      createdAt: new Date().toISOString(),
      read: false,
    }, ...arr]);
  }
}

function generateMock(): AppNotification[] {
  const now = Date.now();
  return [
    { id: 'n1', type: 'ORDER_UPDATE',  title: 'Commande CMD-A8X12 confirmée', body: 'Kassim Ouédraogo a confirmé votre commande · livraison prévue samedi.', createdAt: new Date(now - 2 * 3600000).toISOString(),  read: false, link: '/orders/CMD-A8X12/tracking', actorName: 'Kassim Ouédraogo' },
    { id: 'n2', type: 'MESSAGE',       title: 'Nouveau message',               body: 'Awa Sankara : « Je peux livrer lundi matin ? »',                       createdAt: new Date(now - 5 * 3600000).toISOString(),  read: false, link: '/messaging/2', actorName: 'Awa Sankara' },
    { id: 'n3', type: 'REVIEW',        title: 'Nouvel avis reçu · 5★',         body: 'Fatim Compaoré a laissé un avis 5 étoiles sur vos poulets bicyclette.', createdAt: new Date(now - 18 * 3600000).toISOString(), read: false, link: '/profile/eleveur/1#avis', actorName: 'Fatim Compaoré' },
    { id: 'n4', type: 'CERTIFICATION', title: 'Certification halal validée',   body: 'Votre lot L-2026-041 est maintenant certifié halal.',                   createdAt: new Date(now - 2 * 86400000).toISOString(), read: true,  link: '/halal/L-2026-041/checklist' },
    { id: 'n5', type: 'MFA_REMINDER',  title: 'Activez la 2FA sur votre compte', body: 'Complétez votre configuration de sécurité : ajoutez au moins une PassKey.', createdAt: new Date(now - 3 * 86400000).toISOString(), read: true,  link: '/profile/mfa' },
    { id: 'n6', type: 'SYSTEM',        title: 'Mise à jour Poulets BF',         body: 'La nouvelle version 1.2 introduit le suivi en temps réel des livraisons.', createdAt: new Date(now - 5 * 86400000).toISOString(), read: true },
  ];
}
