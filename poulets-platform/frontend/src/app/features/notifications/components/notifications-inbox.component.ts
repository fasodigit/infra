// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { CommonModule, DatePipe } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { RouterLink } from '@angular/router';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';
import { MatMenuModule } from '@angular/material/menu';
import { EmptyStateComponent } from '@shared/components/empty-state/empty-state.component';
import { NotificationsService, AppNotification, NotificationType } from '../services/notifications.service';

@Component({
  selector: 'app-notifications-inbox',
  standalone: true,
  imports: [CommonModule, DatePipe, FormsModule, RouterLink, MatIconModule, MatButtonModule, MatMenuModule, EmptyStateComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="page" data-testid="notifications-page">
      <header>
        <div>
          <h1>Notifications</h1>
          <p data-testid="notifications-detail-field-count">{{ svc.unreadCount() }} non lue{{ svc.unreadCount() > 1 ? 's' : '' }} · {{ svc.items().length }} au total</p>
        </div>
        <div class="actions">
          <button mat-stroked-button type="button" (click)="svc.markAllRead()" [disabled]="svc.unreadCount() === 0"
                  data-testid="notifications-action-mark-all-read">
            <mat-icon>done_all</mat-icon> Tout marquer comme lu
          </button>
          <button mat-stroked-button color="warn" type="button" (click)="svc.deleteAll()" [disabled]="svc.items().length === 0"
                  data-testid="notifications-action-clear-all">
            <mat-icon>delete_sweep</mat-icon> Tout supprimer
          </button>
        </div>
      </header>

      <div class="tabs" role="tablist" data-testid="notifications-filter-tabs">
        <button [class.active]="tab() === 'all'" (click)="tab.set('all')"
                data-testid="notifications-filter-all">
          Toutes ({{ svc.items().length }})
        </button>
        <button [class.active]="tab() === 'unread'" (click)="tab.set('unread')"
                data-testid="notifications-filter-unread">
          Non lues ({{ svc.unreadCount() }})
        </button>
        @for (t of TYPES; track t.value) {
          <button [class.active]="tab() === t.value" (click)="tab.set(t.value)"
                  [attr.data-testid]="'notifications-filter-' + t.value.toLowerCase()">
            <mat-icon>{{ t.icon }}</mat-icon> {{ t.label }}
          </button>
        }
      </div>

      @if (filtered().length === 0) {
        <app-empty-state icon="notifications_off" title="Aucune notification" data-testid="notifications-empty" />
      } @else {
        <ul class="items" data-testid="notifications-list">
          @for (n of filtered(); track n.id) {
            <li [class.unread]="!n.read"
                [attr.data-testid]="'notifications-list-item-' + n.id">
              <span class="badge" [class]="'badge--' + n.type.toLowerCase()">
                <mat-icon>{{ iconFor(n.type) }}</mat-icon>
              </span>
              <div class="body">
                <strong>{{ n.title }}</strong>
                <p>{{ n.body }}</p>
                <div class="meta">
                  @if (n.actorName) { <span>{{ n.actorName }} · </span> }
                  <time>{{ n.createdAt | date:'short' }}</time>
                  @if (n.link) {
                    <a [routerLink]="n.link" (click)="svc.markRead(n.id)">Voir →</a>
                  }
                </div>
              </div>
              <button mat-icon-button [matMenuTriggerFor]="menu" aria-label="Actions">
                <mat-icon>more_vert</mat-icon>
              </button>
              <mat-menu #menu="matMenu">
                @if (!n.read) {
                  <button mat-menu-item (click)="svc.markRead(n.id)"
                          [attr.data-testid]="'notifications-action-mark-read-' + n.id">
                    <mat-icon>mark_email_read</mat-icon><span>Marquer comme lu</span>
                  </button>
                }
                <button mat-menu-item (click)="svc.delete(n.id)"
                        [attr.data-testid]="'notifications-action-delete-' + n.id">
                  <mat-icon>delete</mat-icon><span>Supprimer</span>
                </button>
              </mat-menu>
            </li>
          }
        </ul>
      }
    </section>
  `,
  styles: [`
    :host { display: block; background: var(--faso-bg); min-height: 100vh; }
    .page {
      max-width: 920px;
      margin: 0 auto;
      padding: var(--faso-space-6) var(--faso-space-4) var(--faso-space-12);
    }
    header {
      display: flex;
      justify-content: space-between;
      align-items: flex-end;
      gap: var(--faso-space-3);
      margin-bottom: var(--faso-space-5);
      flex-wrap: wrap;
    }
    header h1 { margin: 0; font-size: var(--faso-text-3xl); font-weight: var(--faso-weight-bold); }
    header p { margin: 4px 0 0; color: var(--faso-text-muted); }
    .actions { display: flex; gap: var(--faso-space-2); flex-wrap: wrap; }

    .tabs {
      display: flex;
      flex-wrap: wrap;
      gap: 4px;
      padding: 4px;
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-pill);
      margin-bottom: var(--faso-space-4);
    }
    .tabs button {
      display: inline-flex;
      align-items: center;
      gap: 4px;
      padding: 6px 12px;
      border: none;
      background: transparent;
      border-radius: var(--faso-radius-pill);
      cursor: pointer;
      color: var(--faso-text-muted);
      font-size: var(--faso-text-sm);
      font-weight: var(--faso-weight-medium);
    }
    .tabs button mat-icon { font-size: 16px; width: 16px; height: 16px; }
    .tabs button.active {
      background: var(--faso-primary-600);
      color: var(--faso-text-inverse);
    }

    .items { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: var(--faso-space-2); }
    .items li {
      display: grid;
      grid-template-columns: auto 1fr auto;
      gap: var(--faso-space-3);
      align-items: flex-start;
      padding: var(--faso-space-3) var(--faso-space-4);
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-lg);
      transition: border-color var(--faso-duration-fast) var(--faso-ease-standard);
    }
    .items li.unread {
      border-left: 4px solid var(--faso-primary-600);
      background: var(--faso-primary-50);
    }
    .badge {
      display: inline-flex;
      width: 40px; height: 40px;
      border-radius: 50%;
      align-items: center;
      justify-content: center;
      background: var(--faso-primary-50);
      color: var(--faso-primary-700);
    }
    .badge--message       { background: var(--faso-info-bg);     color: var(--faso-info); }
    .badge--order_update  { background: var(--faso-success-bg);  color: var(--faso-success); }
    .badge--review        { background: var(--faso-accent-100);  color: var(--faso-accent-800); }
    .badge--certification { background: var(--faso-primary-100); color: var(--faso-primary-700); }
    .badge--mfa_reminder  { background: var(--faso-warning-bg);  color: var(--faso-warning); }
    .badge--system        { background: var(--faso-surface-alt); color: var(--faso-text-muted); }

    .body strong { display: block; }
    .body p { margin: 4px 0 6px; color: var(--faso-text-muted); }
    .meta { color: var(--faso-text-subtle); font-size: var(--faso-text-xs); display: inline-flex; gap: 6px; align-items: center; flex-wrap: wrap; }
    .meta a { color: var(--faso-primary-700); font-weight: var(--faso-weight-semibold); }
  `],
})
export class NotificationsInboxComponent {
  readonly svc = inject(NotificationsService);
  readonly tab = signal<'all' | 'unread' | NotificationType>('all');

  readonly TYPES: { value: NotificationType; label: string; icon: string }[] = [
    { value: 'ORDER_UPDATE',  label: 'Commandes',   icon: 'receipt_long' },
    { value: 'MESSAGE',       label: 'Messages',    icon: 'chat' },
    { value: 'REVIEW',        label: 'Avis',        icon: 'star' },
    { value: 'CERTIFICATION', label: 'Certif',      icon: 'verified' },
    { value: 'SYSTEM',        label: 'Système',     icon: 'campaign' },
  ];

  readonly filtered = computed(() => {
    const t = this.tab();
    return this.svc.items().filter((n) => {
      if (t === 'all') return true;
      if (t === 'unread') return !n.read;
      return n.type === t;
    });
  });

  iconFor(t: NotificationType): string {
    return this.TYPES.find((x) => x.value === t)?.icon ?? 'notifications';
  }
}
