// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, OnInit, inject, signal, computed } from '@angular/core';
import { CommonModule, DatePipe, DecimalPipe } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { RouterLink } from '@angular/router';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';

import { StatCardComponent } from '@shared/components/stat-card/stat-card.component';
import { SectionHeaderComponent } from '@shared/components/section-header/section-header.component';
import { LoadingComponent } from '@shared/components/loading/loading.component';
import { TemporalWorkflowsService } from '../services/temporal-workflows.service';
import { WorkflowExecution, WorkflowStatus, WorkflowType, WorkflowLatency } from '../models';

@Component({
  selector: 'app-workflow-monitoring',
  standalone: true,
  imports: [
    CommonModule, DatePipe, DecimalPipe, FormsModule, RouterLink,
    MatIconModule, MatButtonModule,
    StatCardComponent, SectionHeaderComponent, LoadingComponent,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="page">
      <header>
        <div>
          <h1>Monitoring Temporal.io</h1>
          <p>Orchestrateur de workflows longs · sagas compensatoires · four-eyes humains</p>
        </div>
        <div class="actions">
          <a mat-stroked-button [href]="svc.temporalUiUrl" target="_blank" rel="noopener">
            <mat-icon>open_in_new</mat-icon> Temporal UI
          </a>
        </div>
      </header>

      <div class="kpis">
        <app-stat-card
          icon="route"
          label="Workflows actifs"
          [value]="activeCount()"
          sublabel="En cours d'exécution"
        />
        <app-stat-card
          icon="task_alt"
          label="Terminés 24h"
          [value]="completed24h()"
          sublabel="Succès"
        />
        <app-stat-card
          icon="error_outline"
          label="Échecs 24h"
          [value]="failed24h()"
          [status]="failed24h() > 0 ? 'critical' : 'healthy'"
          sublabel="Retry automatiques ou manuels"
        />
        <app-stat-card
          icon="schedule"
          label="P99 orders"
          [value]="orderP99Days()"
          unit="jours"
          sublabel="Du placement à la clôture"
        />
      </div>

      <app-section-header title="Latences par workflow" kicker="P99 7 jours glissants" />
      <div class="latencies">
        @for (l of latencies(); track l.type) {
          <article class="lat">
            <header>
              <strong>{{ l.type }}</strong>
              <span class="count">{{ l.count24h }} / 24h</span>
            </header>
            <div class="bars">
              <div class="bar">
                <span class="label">P50</span>
                <div class="track"><div class="fill p50" [style.width.%]="pctP50(l)"></div></div>
                <span class="val">{{ humanDuration(l.p50Ms) }}</span>
              </div>
              <div class="bar">
                <span class="label">P95</span>
                <div class="track"><div class="fill p95" [style.width.%]="pctP95(l)"></div></div>
                <span class="val">{{ humanDuration(l.p95Ms) }}</span>
              </div>
              <div class="bar">
                <span class="label">P99</span>
                <div class="track"><div class="fill p99" [style.width.%]="100"></div></div>
                <span class="val">{{ humanDuration(l.p99Ms) }}</span>
              </div>
            </div>
            <footer>
              <span [class.warn]="l.failRate24h > 0.05">
                <mat-icon>{{ l.failRate24h > 0.05 ? 'warning' : 'check_circle' }}</mat-icon>
                {{ (l.failRate24h * 100) | number:'1.0-1' }}% échecs
              </span>
            </footer>
          </article>
        }
      </div>

      <app-section-header title="Workflows en cours" kicker="Actifs" />

      <div class="filters">
        <label class="field">
          <span>Type</span>
          <select [(ngModel)]="typeFilter" (ngModelChange)="reload()">
            <option value="">Tous</option>
            <option value="OrderWorkflow">OrderWorkflow</option>
            <option value="HalalCertificationWorkflow">HalalCertificationWorkflow</option>
            <option value="MfaOnboardingWorkflow">MfaOnboardingWorkflow</option>
            <option value="LotGrowthWorkflow">LotGrowthWorkflow</option>
            <option value="DisputeSaga">DisputeSaga</option>
            <option value="AnnouncePublishWorkflow">AnnouncePublishWorkflow</option>
          </select>
        </label>
        <label class="field">
          <span>Statut</span>
          <select [(ngModel)]="statusFilter" (ngModelChange)="reload()">
            <option value="">Tous</option>
            <option value="running">Running</option>
            <option value="completed">Completed</option>
            <option value="failed">Failed</option>
            <option value="canceled">Canceled</option>
            <option value="timed_out">Timed out</option>
          </select>
        </label>
      </div>

      @if (loading()) {
        <app-loading message="Chargement…" />
      } @else if (workflows().length === 0) {
        <p class="empty">Aucun workflow pour ces critères.</p>
      } @else {
        <ul class="wflist">
          @for (w of workflows(); track w.id) {
            <li>
              <div class="left">
                <mat-icon [class]="'icon--' + w.status">{{ iconFor(w.status) }}</mat-icon>
                <div>
                  <a [routerLink]="[w.id]"><strong>{{ w.type }}</strong></a>
                  <small><code>{{ w.id }}</code> · queue: {{ w.taskQueue }}</small>
                </div>
              </div>
              <div class="mid">
                <span class="status" [class]="'status--' + w.status">{{ w.status }}</span>
                @if (w.retries > 0) { <span class="retry">{{ w.retries }} retries</span> }
              </div>
              <div class="right">
                @if (w.actorName) { <small>{{ w.actorName }}</small> }
                <time>{{ w.startedAt | date:'short' }}</time>
              </div>
            </li>
          }
        </ul>
      }
    </section>
  `,
  styles: [`
    :host { display: block; }
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

    .kpis {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
      gap: var(--faso-space-4);
      margin-bottom: var(--faso-space-8);
    }

    .latencies {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
      gap: var(--faso-space-3);
      margin-bottom: var(--faso-space-8);
    }
    .lat {
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
      padding: var(--faso-space-4);
    }
    .lat header {
      display: flex;
      justify-content: space-between;
      margin: 0 0 var(--faso-space-3);
    }
    .lat .count {
      color: var(--faso-text-muted);
      font-size: var(--faso-text-sm);
    }
    .bars { display: flex; flex-direction: column; gap: 6px; }
    .bar {
      display: grid;
      grid-template-columns: 36px 1fr auto;
      gap: 8px;
      align-items: center;
      font-size: var(--faso-text-sm);
    }
    .bar .label { color: var(--faso-text-muted); font-family: var(--faso-font-mono); font-size: var(--faso-text-xs); }
    .bar .val { color: var(--faso-text); font-weight: var(--faso-weight-medium); }
    .track {
      height: 8px;
      background: var(--faso-surface-alt);
      border-radius: var(--faso-radius-pill);
      overflow: hidden;
    }
    .fill {
      height: 100%;
      border-radius: inherit;
      transition: width var(--faso-duration-slow) var(--faso-ease-standard);
    }
    .fill.p50 { background: var(--faso-success); }
    .fill.p95 { background: var(--faso-warning); }
    .fill.p99 { background: var(--faso-danger); }
    .lat footer {
      display: flex;
      margin-top: var(--faso-space-3);
      padding-top: var(--faso-space-2);
      border-top: 1px solid var(--faso-border);
    }
    .lat footer span { display: inline-flex; align-items: center; gap: 4px; font-size: var(--faso-text-xs); color: var(--faso-text-muted); }
    .lat footer span mat-icon { font-size: 14px; width: 14px; height: 14px; color: var(--faso-success); }
    .lat footer .warn mat-icon { color: var(--faso-warning); }

    .filters {
      display: flex;
      gap: var(--faso-space-3);
      margin-bottom: var(--faso-space-3);
      flex-wrap: wrap;
    }
    .field { display: flex; flex-direction: column; gap: 4px; min-width: 200px; }
    .field span { font-size: var(--faso-text-xs); font-weight: var(--faso-weight-semibold); color: var(--faso-text-muted); text-transform: uppercase; }
    .field select {
      padding: 8px 12px;
      border: 1px solid var(--faso-border-strong);
      border-radius: var(--faso-radius-md);
      font-family: inherit;
      font-size: var(--faso-text-sm);
    }

    .empty { padding: var(--faso-space-10); text-align: center; color: var(--faso-text-muted); }

    .wflist { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: var(--faso-space-2); }
    .wflist li {
      display: grid;
      grid-template-columns: 2fr auto 1fr;
      gap: var(--faso-space-3);
      align-items: center;
      padding: var(--faso-space-3) var(--faso-space-4);
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-lg);
    }
    .left { display: flex; gap: var(--faso-space-3); align-items: center; }
    .left a { color: var(--faso-text); text-decoration: none; }
    .left a:hover { color: var(--faso-primary-700); }
    .left small { display: block; color: var(--faso-text-muted); font-size: var(--faso-text-xs); }
    .left code {
      font-family: var(--faso-font-mono);
      background: var(--faso-surface-alt);
      padding: 1px 4px;
      border-radius: var(--faso-radius-sm);
    }

    .icon--running   { color: var(--faso-info); }
    .icon--completed { color: var(--faso-success); }
    .icon--failed    { color: var(--faso-danger); }
    .icon--canceled  { color: var(--faso-text-muted); }
    .icon--timed_out { color: var(--faso-warning); }

    .status {
      padding: 2px 10px;
      border-radius: var(--faso-radius-pill);
      font-size: var(--faso-text-xs);
      font-weight: var(--faso-weight-semibold);
      text-transform: uppercase;
      letter-spacing: 0.04em;
    }
    .status--running   { background: var(--faso-info-bg);    color: var(--faso-info); }
    .status--completed { background: var(--faso-success-bg); color: var(--faso-success); }
    .status--failed    { background: var(--faso-danger-bg);  color: var(--faso-danger); }
    .status--canceled  { background: var(--faso-surface-alt); color: var(--faso-text-muted); }
    .status--timed_out { background: var(--faso-warning-bg); color: var(--faso-warning); }

    .retry {
      font-size: var(--faso-text-xs);
      color: var(--faso-warning);
      font-weight: var(--faso-weight-semibold);
    }

    .right { display: flex; flex-direction: column; text-align: right; }
    .right small { color: var(--faso-text-muted); }
    .right time { color: var(--faso-text-subtle); font-size: var(--faso-text-xs); }
  `],
})
export class WorkflowMonitoringComponent implements OnInit {
  readonly svc = inject(TemporalWorkflowsService);

  readonly workflows = signal<WorkflowExecution[]>([]);
  readonly latencies = signal<WorkflowLatency[]>([]);
  readonly loading = signal(true);

  typeFilter: '' | WorkflowType = '';
  statusFilter: '' | WorkflowStatus = '';

  readonly activeCount = computed(() => this.workflows().filter((w) => w.status === 'running').length);
  readonly completed24h = () => MOCK_COUNT_24H.completed;
  readonly failed24h = () => MOCK_COUNT_24H.failed;
  readonly orderP99Days = computed(() => {
    const l = this.latencies().find((x) => x.type === 'OrderWorkflow');
    return l ? (l.p99Ms / 86400000).toFixed(1) : '—';
  });

  ngOnInit(): void {
    this.svc.latencyStats().subscribe((arr) => this.latencies.set(arr));
    this.reload();
  }

  reload(): void {
    this.loading.set(true);
    this.svc.list({
      type: (this.typeFilter || undefined) as WorkflowType,
      status: (this.statusFilter || undefined) as WorkflowStatus,
    }).subscribe({
      next: (arr) => { this.workflows.set(arr); this.loading.set(false); },
      error: () => this.loading.set(false),
    });
  }

  iconFor(s: WorkflowStatus): string {
    switch (s) {
      case 'running':    return 'sync';
      case 'completed':  return 'check_circle';
      case 'failed':     return 'error';
      case 'canceled':   return 'block';
      case 'timed_out':  return 'hourglass_disabled';
    }
  }

  humanDuration(ms: number): string {
    if (ms < 1000) return ms + ' ms';
    if (ms < 60000) return (ms / 1000).toFixed(1) + ' s';
    if (ms < 3600000) return Math.round(ms / 60000) + ' min';
    if (ms < 86400000) return (ms / 3600000).toFixed(1) + ' h';
    return (ms / 86400000).toFixed(1) + ' j';
  }

  maxP99(): number {
    return Math.max(1, ...this.latencies().map((l) => l.p99Ms));
  }
  pctP50(l: WorkflowLatency): number { return (l.p50Ms / l.p99Ms) * 100; }
  pctP95(l: WorkflowLatency): number { return (l.p95Ms / l.p99Ms) * 100; }
}

const MOCK_COUNT_24H = { completed: 264, failed: 8 };
