// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, OnInit, inject, signal } from '@angular/core';
import { CommonModule, DatePipe } from '@angular/common';
import { ActivatedRoute, Router, RouterLink } from '@angular/router';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';
import { MatSnackBar } from '@angular/material/snack-bar';
import { LoadingComponent } from '@shared/components/loading/loading.component';
import { SectionHeaderComponent } from '@shared/components/section-header/section-header.component';
import { TemporalWorkflowsService } from '../services/temporal-workflows.service';
import { WorkflowExecution, WorkflowHistoryEvent, ActivityRun } from '../models';

@Component({
  selector: 'app-workflow-detail',
  standalone: true,
  imports: [
    CommonModule, DatePipe, RouterLink, MatIconModule, MatButtonModule,
    LoadingComponent, SectionHeaderComponent,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="page">
      <a mat-button routerLink="/admin/workflows" class="back">
        <mat-icon>arrow_back</mat-icon> Retour aux workflows
      </a>

      @if (loading()) {
        <app-loading message="Chargement…" />
      } @else if (wf(); as w) {
        <header class="head">
          <div>
            <h1>{{ w.type }}</h1>
            <p>
              <code>{{ w.id }}</code> · queue: {{ w.taskQueue }}
              @if (w.actorName) { · <strong>{{ w.actorName }}</strong> }
            </p>
            <span class="status" [class]="'status--' + w.status">{{ w.status }}</span>
            @if (w.retries > 0) { <span class="retry">{{ w.retries }} retries</span> }
          </div>
          <div class="actions">
            <a mat-stroked-button [href]="svc.temporalUiLink(w.id)" target="_blank" rel="noopener">
              <mat-icon>open_in_new</mat-icon> Ouvrir dans Temporal UI
            </a>
            @if (w.status === 'running') {
              <button mat-stroked-button type="button" (click)="signal(w)">
                <mat-icon>send</mat-icon> Envoyer signal
              </button>
              <button mat-stroked-button color="warn" type="button" (click)="cancel(w)">
                <mat-icon>cancel</mat-icon> Annuler
              </button>
              <button mat-raised-button color="warn" type="button" (click)="terminate(w)">
                <mat-icon>stop</mat-icon> Terminer (force)
              </button>
            }
          </div>
        </header>

        <div class="grid">
          <section class="card">
            <app-section-header title="Activités" kicker="Exécutions" />
            @if (activities().length === 0) {
              <p class="empty">Aucune activité.</p>
            } @else {
              <ol class="activities">
                @for (a of activities(); track a.id) {
                  <li [class]="'a--' + a.status">
                    <span class="dot"><mat-icon>{{ actIcon(a.status) }}</mat-icon></span>
                    <div>
                      <strong>{{ a.name }}</strong>
                      <small>
                        Attempt #{{ a.attempt }}
                        @if (a.durationMs) { · durée {{ a.durationMs }}ms }
                      </small>
                      @if (a.error) { <span class="err">{{ a.error }}</span> }
                    </div>
                    <time>{{ a.startedAt | date:'short' }}</time>
                  </li>
                }
              </ol>
            }
          </section>

          <section class="card">
            <app-section-header title="Historique événements" kicker="Temporal events" />
            <ol class="events">
              @for (e of history(); track e.id) {
                <li>
                  <time>{{ e.timestamp | date:'mediumTime' }}</time>
                  <code>{{ e.eventType }}</code>
                  @if (e.payload) { <pre>{{ e.payload | json }}</pre> }
                </li>
              }
            </ol>
          </section>
        </div>
      } @else {
        <p class="empty">Workflow introuvable.</p>
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
      gap: var(--faso-space-4);
      margin: var(--faso-space-2) 0 var(--faso-space-6);
      flex-wrap: wrap;
    }
    .head h1 { margin: 0; font-size: var(--faso-text-2xl); font-weight: var(--faso-weight-bold); }
    .head p { margin: 4px 0 8px; color: var(--faso-text-muted); }
    .head code { font-family: var(--faso-font-mono); background: var(--faso-surface-alt); padding: 2px 6px; border-radius: var(--faso-radius-sm); font-size: var(--faso-text-xs); }

    .status {
      padding: 3px 12px;
      border-radius: var(--faso-radius-pill);
      font-size: var(--faso-text-sm);
      font-weight: var(--faso-weight-semibold);
      text-transform: uppercase;
      letter-spacing: 0.04em;
      margin-right: 8px;
    }
    .status--running   { background: var(--faso-info-bg);    color: var(--faso-info); }
    .status--completed { background: var(--faso-success-bg); color: var(--faso-success); }
    .status--failed    { background: var(--faso-danger-bg);  color: var(--faso-danger); }
    .retry {
      color: var(--faso-warning);
      font-size: var(--faso-text-sm);
      font-weight: var(--faso-weight-semibold);
    }

    .actions { display: flex; gap: var(--faso-space-2); flex-wrap: wrap; }

    .grid {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: var(--faso-space-4);
    }
    @media (max-width: 1199px) { .grid { grid-template-columns: 1fr; } }

    .card {
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
      padding: var(--faso-space-4);
    }

    .empty { padding: var(--faso-space-6) 0; text-align: center; color: var(--faso-text-muted); }

    .activities { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: var(--faso-space-2); }
    .activities li {
      display: grid;
      grid-template-columns: auto 1fr auto;
      gap: var(--faso-space-3);
      align-items: flex-start;
      padding: var(--faso-space-3);
      background: var(--faso-surface-alt);
      border-radius: var(--faso-radius-md);
    }
    .activities .dot {
      display: inline-flex;
      width: 32px; height: 32px;
      border-radius: 50%;
      align-items: center;
      justify-content: center;
    }
    .a--completed .dot { background: var(--faso-success-bg); color: var(--faso-success); }
    .a--running   .dot { background: var(--faso-info-bg);    color: var(--faso-info); }
    .a--failed    .dot { background: var(--faso-danger-bg);  color: var(--faso-danger); }
    .a--retried   .dot { background: var(--faso-warning-bg); color: var(--faso-warning); }
    .activities small { color: var(--faso-text-muted); display: block; }
    .activities .err {
      display: block;
      color: var(--faso-danger);
      font-size: var(--faso-text-sm);
      margin-top: 4px;
    }
    .activities time { color: var(--faso-text-subtle); font-size: var(--faso-text-xs); }

    .events { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: var(--faso-space-2); }
    .events li {
      padding: var(--faso-space-2) var(--faso-space-3);
      background: var(--faso-surface-alt);
      border-radius: var(--faso-radius-md);
      font-size: var(--faso-text-sm);
    }
    .events time {
      color: var(--faso-text-subtle);
      font-family: var(--faso-font-mono);
      font-size: var(--faso-text-xs);
      margin-right: 6px;
    }
    .events code {
      font-family: var(--faso-font-mono);
      color: var(--faso-primary-700);
      background: var(--faso-primary-50);
      padding: 1px 6px;
      border-radius: var(--faso-radius-sm);
      font-weight: var(--faso-weight-semibold);
    }
    .events pre {
      margin: 4px 0 0;
      padding: 6px 8px;
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-sm);
      font-size: var(--faso-text-xs);
      overflow-x: auto;
    }
  `],
})
export class WorkflowDetailComponent implements OnInit {
  private readonly route = inject(ActivatedRoute);
  private readonly router = inject(Router);
  readonly svc = inject(TemporalWorkflowsService);
  private readonly snack = inject(MatSnackBar);

  readonly wf = signal<WorkflowExecution | null>(null);
  readonly activities = signal<ActivityRun[]>([]);
  readonly history = signal<WorkflowHistoryEvent[]>([]);
  readonly loading = signal(true);

  ngOnInit(): void {
    const id = this.route.snapshot.paramMap.get('id');
    if (!id) return;
    this.svc.get(id).subscribe({
      next: (w) => { this.wf.set(w); this.loading.set(false); },
      error: () => this.loading.set(false),
    });
    this.svc.activities(id).subscribe((arr) => this.activities.set(arr));
    this.svc.history(id).subscribe((arr) => this.history.set(arr));
  }

  signal(w: WorkflowExecution): void {
    this.svc.signal(w.id, 'adminSignal', { origin: 'admin-ui' }).subscribe(() => {
      this.snack.open('Signal envoyé', 'OK', { duration: 2500 });
    });
  }

  cancel(w: WorkflowExecution): void {
    this.svc.cancel(w.id, 'admin-action').subscribe(() => {
      this.snack.open('Workflow annulé', 'OK', { duration: 2500 });
    });
  }

  terminate(w: WorkflowExecution): void {
    this.svc.terminate(w.id, 'manual terminate').subscribe(() => {
      this.snack.open('Workflow terminé (force)', 'OK', { duration: 2500 });
    });
  }

  actIcon(s: string): string {
    switch (s) {
      case 'completed':  return 'check';
      case 'running':    return 'sync';
      case 'failed':     return 'close';
      case 'retried':    return 'refresh';
      case 'pending':    return 'schedule';
    }
    return 'radio_button_unchecked';
  }
}
