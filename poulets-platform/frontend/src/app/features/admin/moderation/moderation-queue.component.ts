// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, computed, inject, signal } from '@angular/core';
import { CommonModule, DatePipe } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { RouterLink } from '@angular/router';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';
import { SectionHeaderComponent } from '@shared/components/section-header/section-header.component';
import { ModerationService } from './services/moderation.service';
import { ModerationItem, ModerationStatus, Priority } from './models';

const STATUS_LABELS: Record<ModerationStatus, string> = {
  pending:    'À traiter',
  in_review:  'En cours',
  approved:   'Validé',
  rejected:   'Refusé',
  escalated:  'Escaladé',
};

@Component({
  selector: 'app-moderation-queue',
  standalone: true,
  imports: [CommonModule, DatePipe, FormsModule, RouterLink, MatIconModule, MatButtonModule, SectionHeaderComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="page">
      <header>
        <div>
          <h1>Modération</h1>
          <p>{{ svc.pendingCount() }} à traiter · {{ svc.inReviewCount() }} en cours</p>
        </div>
      </header>

      <div class="tabs" role="tablist">
        @for (t of TABS; track t.value) {
          <button
            role="tab"
            [class.active]="tab() === t.value"
            (click)="tab.set(t.value)"
          >
            @if (t.icon) { <mat-icon>{{ t.icon }}</mat-icon> }
            {{ t.label }} ({{ countByStatus(t.value) }})
          </button>
        }
      </div>

      <div class="filters">
        <label class="field">
          <span>Recherche</span>
          <input type="search" [(ngModel)]="search" placeholder="Titre, auteur, région…">
        </label>
        <label class="field">
          <span>Priorité</span>
          <select [(ngModel)]="priorityFilter">
            <option value="">Toutes</option>
            <option value="P0">P0 (critique)</option>
            <option value="P1">P1 (haute)</option>
            <option value="P2">P2 (normale)</option>
          </select>
        </label>
      </div>

      @if (filtered().length === 0) {
        <p class="empty">Aucun élément ne correspond.</p>
      } @else {
        <ul class="cards">
          @for (m of filtered(); track m.id) {
            <li>
              <div class="left">
                <span class="prio" [class]="'prio--' + m.priority.toLowerCase()">{{ m.priority }}</span>
                <mat-icon [class]="'icon--' + m.type.toLowerCase()">{{ iconFor(m.type) }}</mat-icon>
              </div>

              <div class="body">
                <a [routerLink]="[m.id]"><strong>{{ m.title }}</strong></a>
                <p>{{ m.summary }}</p>
                <div class="meta">
                  <span>{{ m.authorName }}</span>
                  @if (m.region) { · <span>{{ m.region }}</span> }
                  · <time>{{ m.createdAt | date:'short' }}</time>
                  @if (m.requiresFourEyes) {
                    · <span class="eye-tag"><mat-icon>visibility</mat-icon> Four-eyes</span>
                  }
                </div>
              </div>

              <div class="right">
                <span class="status" [class]="'status--' + m.status">{{ STATUS_LABELS[m.status] }}</span>
                @if (m.lockedBy) {
                  <span class="lock"><mat-icon>lock</mat-icon> {{ m.lockedBy }}</span>
                }
                @if (m.slaRemainingMin > 0) {
                  <span class="sla" [class.warn]="m.slaRemainingMin < 60">
                    <mat-icon>schedule</mat-icon>
                    {{ slaText(m.slaRemainingMin) }}
                  </span>
                }
                <a mat-stroked-button [routerLink]="[m.id]">
                  {{ m.status === 'pending' ? 'Prendre' : 'Ouvrir' }} →
                </a>
              </div>
            </li>
          }
        </ul>
      }
    </section>
  `,
  styles: [`
    :host { display: block; }
    header { margin-bottom: var(--faso-space-5); }
    header h1 { margin: 0; font-size: var(--faso-text-3xl); font-weight: var(--faso-weight-bold); }
    header p { margin: 4px 0 0; color: var(--faso-text-muted); }

    .tabs {
      display: flex; gap: 4px; padding: 4px;
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-pill);
      margin-bottom: var(--faso-space-3);
      flex-wrap: wrap;
    }
    .tabs button {
      display: inline-flex; align-items: center; gap: 4px;
      padding: 6px 14px;
      border: none;
      background: transparent;
      border-radius: var(--faso-radius-pill);
      cursor: pointer;
      color: var(--faso-text-muted);
      font-weight: var(--faso-weight-medium);
    }
    .tabs button mat-icon { font-size: 18px; width: 18px; height: 18px; }
    .tabs button.active { background: var(--faso-primary-600); color: var(--faso-text-inverse); }

    .filters {
      display: grid;
      grid-template-columns: 2fr 1fr;
      gap: var(--faso-space-3);
      margin-bottom: var(--faso-space-4);
    }
    .field { display: flex; flex-direction: column; gap: 4px; }
    .field span { font-size: var(--faso-text-xs); font-weight: var(--faso-weight-semibold); color: var(--faso-text-muted); text-transform: uppercase; }
    .field input, .field select {
      padding: 8px 12px;
      border: 1px solid var(--faso-border-strong);
      border-radius: var(--faso-radius-md);
      font-family: inherit;
      font-size: var(--faso-text-sm);
    }

    .empty { padding: var(--faso-space-10); text-align: center; color: var(--faso-text-muted); }

    .cards { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: var(--faso-space-2); }
    .cards li {
      display: grid;
      grid-template-columns: auto 1fr auto;
      gap: var(--faso-space-3);
      align-items: flex-start;
      padding: var(--faso-space-3) var(--faso-space-4);
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
    }

    .left { display: flex; flex-direction: column; gap: 6px; align-items: center; }
    .prio {
      padding: 2px 8px;
      border-radius: var(--faso-radius-pill);
      font-weight: var(--faso-weight-bold);
      font-size: var(--faso-text-xs);
    }
    .prio--p0 { background: var(--faso-danger-bg); color: var(--faso-danger); border: 1px solid var(--faso-danger); }
    .prio--p1 { background: var(--faso-warning-bg); color: var(--faso-warning); border: 1px solid var(--faso-warning); }
    .prio--p2 { background: var(--faso-surface-alt); color: var(--faso-text-muted); border: 1px solid var(--faso-border); }
    .icon--annonce_new, .icon--annonce_flagged { color: var(--faso-primary-700); }
    .icon--halal_cert_review { color: var(--faso-accent-700); }
    .icon--user_report { color: var(--faso-warning); }
    .icon--review_flagged { color: var(--faso-info); }

    .body a { color: var(--faso-text); text-decoration: none; }
    .body a:hover { color: var(--faso-primary-700); }
    .body strong { display: block; font-size: var(--faso-text-lg); }
    .body p { margin: 4px 0; color: var(--faso-text-muted); }
    .meta { color: var(--faso-text-subtle); font-size: var(--faso-text-xs); display: inline-flex; gap: 4px; align-items: center; flex-wrap: wrap; }
    .eye-tag {
      display: inline-flex; align-items: center; gap: 2px;
      color: var(--faso-accent-800);
      padding: 1px 6px;
      background: var(--faso-accent-100);
      border-radius: var(--faso-radius-pill);
      font-weight: var(--faso-weight-semibold);
    }
    .eye-tag mat-icon { font-size: 12px; width: 12px; height: 12px; }

    .right { display: flex; flex-direction: column; gap: 6px; align-items: flex-end; }
    .status {
      padding: 2px 10px;
      border-radius: var(--faso-radius-pill);
      font-size: var(--faso-text-xs);
      font-weight: var(--faso-weight-semibold);
    }
    .status--pending    { background: var(--faso-warning-bg); color: var(--faso-warning); }
    .status--in_review  { background: var(--faso-info-bg);    color: var(--faso-info); }
    .status--approved   { background: var(--faso-success-bg); color: var(--faso-success); }
    .status--rejected   { background: var(--faso-danger-bg);  color: var(--faso-danger); }
    .status--escalated  { background: var(--faso-accent-100); color: var(--faso-accent-800); }

    .lock, .sla {
      display: inline-flex; align-items: center; gap: 2px;
      font-size: var(--faso-text-xs);
      color: var(--faso-text-muted);
    }
    .lock mat-icon, .sla mat-icon { font-size: 14px; width: 14px; height: 14px; }
    .sla.warn { color: var(--faso-danger); font-weight: var(--faso-weight-semibold); }
  `],
})
export class ModerationQueueComponent {
  readonly svc = inject(ModerationService);

  readonly tab = signal<ModerationStatus>('pending');
  search = '';
  priorityFilter: '' | Priority = '';

  readonly TABS: { value: ModerationStatus; label: string; icon?: string }[] = [
    { value: 'pending',   label: 'À traiter',  icon: 'inbox' },
    { value: 'in_review', label: 'En cours',   icon: 'hourglass_top' },
    { value: 'approved',  label: 'Validés',    icon: 'done_all' },
    { value: 'rejected',  label: 'Refusés',    icon: 'block' },
    { value: 'escalated', label: 'Escaladés',  icon: 'report' },
  ];

  readonly STATUS_LABELS = STATUS_LABELS;

  readonly filtered = computed(() => {
    const t = this.tab();
    const q = this.search.trim().toLowerCase();
    const p = this.priorityFilter;
    return this.svc.items().filter((m) => {
      if (m.status !== t) return false;
      if (p && m.priority !== p) return false;
      if (q) {
        const blob = `${m.title} ${m.summary} ${m.authorName} ${m.region ?? ''}`.toLowerCase();
        if (!blob.includes(q)) return false;
      }
      return true;
    });
  });

  countByStatus(s: ModerationStatus): number {
    return this.svc.items().filter((m) => m.status === s).length;
  }

  iconFor(t: ModerationItem['type']): string {
    switch (t) {
      case 'ANNONCE_NEW':       return 'add_box';
      case 'ANNONCE_FLAGGED':   return 'flag';
      case 'HALAL_CERT_REVIEW': return 'verified';
      case 'USER_REPORT':       return 'person_off';
      case 'REVIEW_FLAGGED':    return 'star_border';
    }
  }

  slaText(min: number): string {
    if (min < 60) return `${min} min`;
    if (min < 1440) return `${Math.round(min / 60)} h`;
    return `${Math.round(min / 1440)} j`;
  }
}
