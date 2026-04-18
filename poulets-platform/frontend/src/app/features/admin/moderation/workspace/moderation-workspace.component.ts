// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, OnInit, OnDestroy, inject, signal, computed } from '@angular/core';
import { CommonModule, DatePipe } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { ActivatedRoute, Router, RouterLink } from '@angular/router';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';
import { MatSnackBar } from '@angular/material/snack-bar';

import { LoadingComponent } from '@shared/components/loading/loading.component';
import { ModerationService } from '../services/moderation.service';
import { ModerationItem } from '../models';

@Component({
  selector: 'app-moderation-workspace',
  standalone: true,
  imports: [CommonModule, DatePipe, FormsModule, RouterLink, MatIconModule, MatButtonModule, LoadingComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="page">
      <a mat-button routerLink="/admin/moderation" class="back">
        <mat-icon>arrow_back</mat-icon> Retour à la queue
      </a>

      @if (loading()) {
        <app-loading message="Chargement…" />
      } @else if (item(); as m) {
        <header class="head">
          <div>
            <h1>{{ m.title }}</h1>
            <p>{{ typeLabel(m.type) }} · {{ m.authorName }} · {{ m.createdAt | date:'medium' }}</p>
          </div>
          <div class="badges">
            <span class="prio" [class]="'prio--' + m.priority.toLowerCase()">{{ m.priority }}</span>
            @if (m.lockedBy) {
              <span class="lock">
                <mat-icon>lock</mat-icon>
                Verrouillé par {{ m.lockedBy }}
                @if (lockRemaining() > 0) { · {{ lockRemaining() }} min }
              </span>
            }
          </div>
        </header>

        <div class="workspace">
          <!-- Zone gauche : attachments -->
          <aside class="attachments">
            <h2>Pièces jointes</h2>
            @if ((m.attachments ?? []).length === 0) {
              <p class="empty">Aucune pièce jointe</p>
            } @else {
              <div class="tabs">
                @for (att of m.attachments; track att.id) {
                  <button [class.active]="selectedAtt() === att.id" (click)="selectedAtt.set(att.id)">
                    <mat-icon>{{ attIcon(att.mime) }}</mat-icon>
                    <span>{{ att.name }}</span>
                  </button>
                }
              </div>
              @if (currentAtt(); as a) {
                <div class="preview">
                  @if (a.mime.startsWith('image/')) {
                    <img [src]="a.url" [alt]="a.name">
                  } @else if (a.mime === 'application/pdf') {
                    <iframe [src]="a.url" title="PDF preview"></iframe>
                  } @else {
                    <p>Prévisualisation indisponible · <a [href]="a.url" target="_blank">Télécharger</a></p>
                  }
                </div>
              }
            }
          </aside>

          <!-- Zone centre : preview + historique -->
          <section class="detail">
            <div class="card">
              <h2>Résumé</h2>
              <p>{{ m.summary }}</p>
            </div>

            <div class="card">
              <h2>Historique</h2>
              <ol class="history">
                @for (h of (m.history ?? []); track h.at) {
                  <li>
                    <time>{{ h.at | date:'short' }}</time>
                    <strong>{{ h.actorName }}</strong>
                    <span [class]="'act--' + h.action">{{ actLabel(h.action) }}</span>
                    @if (h.comment) { <small>— {{ h.comment }}</small> }
                  </li>
                }
              </ol>
            </div>

            @if (m.requiresFourEyes) {
              <div class="card four-eyes">
                <h2><mat-icon>visibility</mat-icon> Four-eyes requis</h2>
                <p>{{ (m.fourEyesApprovals ?? []).length }} / 2 admins ont approuvé.</p>
                <ul>
                  @for (a of (m.fourEyesApprovals ?? []); track a.adminId) {
                    <li><mat-icon>check_circle</mat-icon> {{ a.adminName }} — {{ a.at | date:'short' }}</li>
                  }
                  @for (_ of missingSlots(m); track $index) {
                    <li class="empty"><mat-icon>radio_button_unchecked</mat-icon> En attente</li>
                  }
                </ul>
                <button mat-raised-button color="primary" type="button" (click)="fourEyes(m)"
                        [disabled]="alreadyApproved(m)">
                  Approuver (four-eyes)
                </button>
              </div>
            }
          </section>

          <!-- Zone bas : actions -->
          <aside class="actions">
            <h2>Action</h2>
            <label class="field">
              <span>Commentaire (requis pour refus)</span>
              <textarea [(ngModel)]="comment" rows="3" placeholder="Raison de la décision…"></textarea>
            </label>

            @if (!m.lockedBy || m.lockedBy === adminName()) {
              <div class="buttons">
                <button mat-raised-button color="primary" type="button" (click)="approve(m)">
                  <mat-icon>check</mat-icon> Approuver
                </button>
                <button mat-raised-button color="warn" type="button" (click)="reject(m)" [disabled]="!comment">
                  <mat-icon>close</mat-icon> Refuser
                </button>
                <button mat-stroked-button type="button" (click)="escalate(m)">
                  <mat-icon>call_split</mat-icon> Escalader (four-eyes)
                </button>
                @if (!m.lockedBy) {
                  <button mat-stroked-button type="button" (click)="takeLock(m)">
                    <mat-icon>lock</mat-icon> Prendre en charge (verrouiller 15 min)
                  </button>
                } @else {
                  <button mat-stroked-button type="button" (click)="releaseLock(m)">
                    <mat-icon>lock_open</mat-icon> Libérer le verrou
                  </button>
                }
              </div>
            } @else {
              <p class="warn">
                <mat-icon>info</mat-icon>
                Cet élément est verrouillé par <strong>{{ m.lockedBy }}</strong> jusqu'à {{ m.lockedUntil | date:'short' }}.
              </p>
            }
          </aside>
        </div>
      } @else {
        <p class="empty">Élément introuvable.</p>
      }
    </section>
  `,
  styles: [`
    :host { display: block; }
    .back { margin-left: calc(var(--faso-space-4) * -1); color: var(--faso-text-muted); }

    .head {
      display: flex;
      justify-content: space-between;
      align-items: flex-start;
      gap: var(--faso-space-3);
      margin: var(--faso-space-2) 0 var(--faso-space-5);
      flex-wrap: wrap;
    }
    .head h1 { margin: 0; font-size: var(--faso-text-2xl); font-weight: var(--faso-weight-bold); }
    .head p { margin: 4px 0 0; color: var(--faso-text-muted); }
    .badges { display: flex; gap: var(--faso-space-2); align-items: center; flex-wrap: wrap; }
    .prio {
      padding: 3px 12px;
      border-radius: var(--faso-radius-pill);
      font-weight: var(--faso-weight-bold);
      font-size: var(--faso-text-sm);
    }
    .prio--p0 { background: var(--faso-danger-bg);  color: var(--faso-danger);  border: 1px solid var(--faso-danger); }
    .prio--p1 { background: var(--faso-warning-bg); color: var(--faso-warning); border: 1px solid var(--faso-warning); }
    .prio--p2 { background: var(--faso-surface-alt);color: var(--faso-text-muted); border: 1px solid var(--faso-border); }
    .lock {
      display: inline-flex;
      gap: 4px;
      align-items: center;
      color: var(--faso-text-muted);
      font-size: var(--faso-text-sm);
    }

    .workspace {
      display: grid;
      grid-template-columns: 280px 1fr 320px;
      gap: var(--faso-space-4);
    }
    @media (max-width: 1199px) {
      .workspace { grid-template-columns: 1fr; }
    }

    aside.attachments, .detail > .card, aside.actions {
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
      padding: var(--faso-space-4);
    }
    aside.attachments h2, .detail > .card h2, aside.actions h2 {
      margin: 0 0 var(--faso-space-3);
      font-size: var(--faso-text-base);
      font-weight: var(--faso-weight-semibold);
    }

    .attachments .tabs { display: flex; flex-direction: column; gap: 4px; margin-bottom: var(--faso-space-3); }
    .attachments .tabs button {
      display: flex; align-items: center; gap: 6px;
      padding: 6px 10px;
      background: transparent;
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-md);
      cursor: pointer;
      text-align: left;
      font-size: var(--faso-text-sm);
      color: var(--faso-text);
    }
    .attachments .tabs button mat-icon { font-size: 16px; width: 16px; height: 16px; color: var(--faso-text-muted); }
    .attachments .tabs button.active {
      background: var(--faso-primary-50);
      border-color: var(--faso-primary-500);
      color: var(--faso-primary-700);
    }
    .preview img, .preview iframe {
      width: 100%;
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-md);
    }
    .preview iframe { height: 400px; }

    .detail { display: flex; flex-direction: column; gap: var(--faso-space-3); }
    .history {
      list-style: none;
      padding: 0;
      margin: 0;
      display: flex;
      flex-direction: column;
      gap: var(--faso-space-2);
      border-left: 2px solid var(--faso-border);
      padding-left: var(--faso-space-3);
    }
    .history li {
      display: grid;
      grid-template-columns: 100px 1fr auto;
      gap: 8px;
      align-items: baseline;
      font-size: var(--faso-text-sm);
      padding-bottom: 4px;
    }
    .history time { color: var(--faso-text-subtle); font-size: var(--faso-text-xs); }
    .act--approve      { color: var(--faso-success); font-weight: var(--faso-weight-semibold); }
    .act--reject       { color: var(--faso-danger);  font-weight: var(--faso-weight-semibold); }
    .act--escalate     { color: var(--faso-accent-700); }
    .act--four-eyes-approve { color: var(--faso-primary-700); font-weight: var(--faso-weight-semibold); }
    .history small { color: var(--faso-text-muted); font-style: italic; grid-column: 2; }

    .four-eyes {
      background: var(--faso-accent-100);
      border-color: var(--faso-accent-400);
    }
    .four-eyes h2 {
      display: inline-flex; align-items: center; gap: 4px;
      color: var(--faso-accent-800);
    }
    .four-eyes ul {
      list-style: none; padding: 0; margin: var(--faso-space-2) 0;
      display: flex; flex-direction: column; gap: 4px;
    }
    .four-eyes li { display: inline-flex; align-items: center; gap: 6px; }
    .four-eyes li.empty { color: var(--faso-text-subtle); }
    .four-eyes mat-icon { color: var(--faso-success); }
    .four-eyes li.empty mat-icon { color: var(--faso-text-subtle); }

    .field { display: flex; flex-direction: column; gap: 4px; margin-bottom: var(--faso-space-3); }
    .field span { font-size: var(--faso-text-xs); font-weight: var(--faso-weight-semibold); color: var(--faso-text-muted); text-transform: uppercase; }
    .field textarea {
      padding: 8px 12px;
      border: 1px solid var(--faso-border-strong);
      border-radius: var(--faso-radius-md);
      font-family: inherit;
      resize: vertical;
    }
    .buttons { display: flex; flex-direction: column; gap: var(--faso-space-2); }
    .warn {
      display: inline-flex; gap: 6px; align-items: flex-start;
      background: var(--faso-warning-bg);
      border-left: 3px solid var(--faso-warning);
      padding: var(--faso-space-3);
      border-radius: var(--faso-radius-md);
      color: var(--faso-text);
      font-size: var(--faso-text-sm);
    }
    .warn mat-icon { color: var(--faso-warning); flex-shrink: 0; }

    .empty { padding: var(--faso-space-6) 0; text-align: center; color: var(--faso-text-muted); }
  `],
})
export class ModerationWorkspaceComponent implements OnInit, OnDestroy {
  private readonly route = inject(ActivatedRoute);
  private readonly router = inject(Router);
  private readonly svc = inject(ModerationService);
  private readonly snack = inject(MatSnackBar);

  readonly item = signal<ModerationItem | null>(null);
  readonly loading = signal(true);
  readonly selectedAtt = signal<string | null>(null);
  comment = '';

  readonly adminName = () => this.svc.currentAdminName();

  readonly currentAtt = computed(() => {
    const m = this.item();
    const id = this.selectedAtt();
    return m?.attachments?.find((a) => a.id === id) ?? null;
  });

  private timer: any = null;
  readonly lockRemainingSignal = signal(0);
  lockRemaining = () => this.lockRemainingSignal();

  ngOnInit(): void {
    const id = this.route.snapshot.paramMap.get('id');
    if (!id) return;
    this.svc.get(id).subscribe({
      next: (m) => {
        this.item.set(m);
        if (m?.attachments?.length) this.selectedAtt.set(m.attachments[0]!.id);
        this.loading.set(false);
        this.startTimer();
      },
      error: () => this.loading.set(false),
    });
  }

  ngOnDestroy(): void { if (this.timer) clearInterval(this.timer); }

  private startTimer() {
    if (typeof window === 'undefined') return;
    if (this.timer) clearInterval(this.timer);
    this.timer = setInterval(() => {
      const m = this.item();
      if (!m?.lockedUntil) { this.lockRemainingSignal.set(0); return; }
      const diff = Math.max(0, Math.round((new Date(m.lockedUntil).getTime() - Date.now()) / 60000));
      this.lockRemainingSignal.set(diff);
    }, 1000);
  }

  typeLabel(t: string): string {
    switch (t) {
      case 'ANNONCE_NEW':       return 'Nouvelle annonce';
      case 'ANNONCE_FLAGGED':   return 'Annonce signalée';
      case 'HALAL_CERT_REVIEW': return 'Revue certification halal';
      case 'USER_REPORT':       return 'Signalement utilisateur';
      case 'REVIEW_FLAGGED':    return 'Avis signalé';
    }
    return t;
  }

  attIcon(mime: string): string {
    if (mime.startsWith('image/')) return 'image';
    if (mime === 'application/pdf') return 'picture_as_pdf';
    return 'description';
  }

  actLabel(a: string): string {
    switch (a) {
      case 'create':   return 'Créé';
      case 'lock':     return 'Verrouillé';
      case 'unlock':   return 'Déverrouillé';
      case 'approve':  return 'Approuvé';
      case 'reject':   return 'Refusé';
      case 'escalate': return 'Escaladé';
      case 'comment':  return 'Commentaire';
      case 'four-eyes-approve': return '4-yeux approuvé';
    }
    return a;
  }

  missingSlots(m: ModerationItem): number[] {
    const n = 2 - (m.fourEyesApprovals?.length ?? 0);
    return Array.from({ length: Math.max(0, n) });
  }

  alreadyApproved(m: ModerationItem): boolean {
    return !!m.fourEyesApprovals?.some((a) => a.adminId === this.svc.currentAdminId());
  }

  takeLock(m: ModerationItem): void {
    this.svc.lock(m.id).subscribe((next) => {
      this.item.set(next);
      this.snack.open('Élément verrouillé pour 15 min', 'OK', { duration: 2500 });
    });
  }
  releaseLock(m: ModerationItem): void {
    this.svc.unlock(m.id).subscribe((next) => {
      this.item.set(next);
      this.snack.open('Verrou libéré', 'OK', { duration: 2500 });
    });
  }

  approve(m: ModerationItem): void {
    this.svc.approve(m.id, this.comment || undefined).subscribe((next) => {
      this.item.set(next);
      this.snack.open('Approuvé', 'OK', { duration: 2500 });
      this.router.navigate(['/admin/moderation']);
    });
  }
  reject(m: ModerationItem): void {
    this.svc.reject(m.id, this.comment).subscribe((next) => {
      this.item.set(next);
      this.snack.open('Refusé', 'OK', { duration: 2500 });
      this.router.navigate(['/admin/moderation']);
    });
  }
  escalate(m: ModerationItem): void {
    this.svc.escalate(m.id, this.comment || undefined).subscribe((next) => {
      this.item.set(next);
      this.snack.open('Escaladé vers four-eyes', 'OK', { duration: 2500 });
    });
  }
  fourEyes(m: ModerationItem): void {
    this.svc.fourEyesApprove(m.id).subscribe((next) => {
      this.item.set(next);
      this.snack.open('Votre approbation four-eyes a été enregistrée', 'OK', { duration: 2500 });
    });
  }
}
