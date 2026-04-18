// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, OnInit, inject, computed } from '@angular/core';
import { CommonModule, DatePipe, DecimalPipe } from '@angular/common';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';

import { StatCardComponent } from '@shared/components/stat-card/stat-card.component';
import { SectionHeaderComponent } from '@shared/components/section-header/section-header.component';
import { PlatformHealthStore } from '@core/monitoring/platform-health.store';

@Component({
  selector: 'app-admin-monitoring',
  standalone: true,
  imports: [
    CommonModule, DatePipe, DecimalPipe, MatIconModule, MatButtonModule,
    StatCardComponent, SectionHeaderComponent,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="page">
      <header>
        <h1>Monitoring plateforme</h1>
        <p>
          Dernière synchronisation&nbsp;:
          <strong>{{ health.lastSync() ? (health.lastSync() | date:'medium') : '—' }}</strong>
          @if (health.connected()) {
            <span class="pill ok"><span class="dot"></span> En ligne</span>
          } @else {
            <span class="pill off"><span class="dot"></span> Hors connexion</span>
          }
        </p>
      </header>

      <div class="kpis">
        <app-stat-card
          icon="health_and_safety"
          label="Statut plateforme"
          [value]="health.overallStatus()"
          [status]="statusKind()"
          sublabel="Résumé agrégé des services"
        />
        <app-stat-card
          icon="groups"
          label="Utilisateurs actifs"
          [value]="health.totalActiveUsers()"
          sublabel="7 derniers jours"
        />
        <app-stat-card
          icon="lan"
          label="Services surveillés"
          [value]="health.services().length"
          sublabel="gateway, cache, auth, db, broker, app"
        />
        <app-stat-card
          icon="notification_important"
          label="Alertes critiques"
          [value]="health.criticalAlerts().length"
          [status]="health.criticalAlerts().length > 0 ? 'critical' : 'healthy'"
          sublabel="Non acquittées"
        />
      </div>

      <app-section-header
        title="Santé des services"
        kicker="En direct"
        [subtitle]="health.services().length ? 'Vue agrégée des composants souverains FASO.' : 'Démarrez la stack pour voir les services.'"
      />

      <div class="services">
        @for (s of health.services(); track s.name) {
          <article class="svc" [class]="'svc--' + s.status.toLowerCase()">
            <header>
              <strong>{{ s.name }}</strong>
              <span class="status">
                <span class="status-dot"></span>
                {{ s.status }}
              </span>
            </header>
            <ul class="metrics">
              <li>
                <span>p99</span>
                <strong>{{ s.latencyP99Ms }} ms</strong>
              </li>
              <li>
                <span>req/s</span>
                <strong>{{ s.requestsPerSec | number:'1.0-0' }}</strong>
              </li>
              <li>
                <span>Erreurs</span>
                <strong>{{ s.errorRate * 100 | number:'1.0-2' }}%</strong>
              </li>
              <li>
                <span>Uptime</span>
                <strong>{{ s.uptime }}</strong>
              </li>
            </ul>
            <footer>
              <span class="tag">{{ s.category }}</span>
              <time>{{ s.lastCheck | date:'shortTime' }}</time>
            </footer>
          </article>
        }
      </div>

      <app-section-header title="Alertes récentes" kicker="Dernières 24h" />

      @if (health.alerts().length === 0) {
        <p class="empty">Aucune alerte pour l'instant.</p>
      } @else {
        <ul class="alerts">
          @for (a of health.alerts(); track a.id) {
            <li [class]="'alert--' + a.severity" [class.ack]="a.acknowledged">
              <mat-icon>
                {{ a.severity === 'critical' ? 'error'
                  : a.severity === 'warning' ? 'warning'
                  : 'info' }}
              </mat-icon>
              <div>
                <strong>{{ a.type }} · {{ a.service }}</strong>
                <p>{{ a.message }}</p>
                <time>{{ a.createdAt | date:'medium' }}</time>
                @if (a.acknowledgedBy) {
                  <span class="ack-by">· acquittée par {{ a.acknowledgedBy }}</span>
                }
              </div>
              @if (!a.acknowledged) {
                <button mat-button type="button" (click)="health.acknowledgeAlert(a.id)">
                  Acquitter
                </button>
              }
            </li>
          }
        </ul>
      }
    </section>
  `,
  styles: [`
    :host { display: block; }
    .page { padding: 0 0 var(--faso-space-8); }

    header {
      display: flex;
      justify-content: space-between;
      align-items: flex-end;
      gap: var(--faso-space-3);
      margin-bottom: var(--faso-space-6);
      flex-wrap: wrap;
    }
    header h1 { margin: 0; font-size: var(--faso-text-3xl); font-weight: var(--faso-weight-bold); }
    header p { margin: 4px 0 0; color: var(--faso-text-muted); }

    .pill {
      display: inline-flex;
      align-items: center;
      gap: 4px;
      margin-left: var(--faso-space-2);
      padding: 2px 8px;
      border-radius: var(--faso-radius-pill);
      font-size: var(--faso-text-xs);
      font-weight: var(--faso-weight-semibold);
    }
    .pill .dot { width: 8px; height: 8px; border-radius: 50%; }
    .pill.ok  { background: var(--faso-success-bg); color: var(--faso-success); }
    .pill.ok .dot  { background: var(--faso-success); }
    .pill.off { background: var(--faso-surface-alt); color: var(--faso-text-muted); }
    .pill.off .dot { background: var(--faso-text-subtle); }

    .kpis {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
      gap: var(--faso-space-4);
      margin-bottom: var(--faso-space-10);
    }

    .services {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
      gap: var(--faso-space-3);
      margin-bottom: var(--faso-space-10);
    }
    .svc {
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
      padding: var(--faso-space-4);
      border-left-width: 4px;
    }
    .svc--up       { border-left-color: var(--faso-success); }
    .svc--degraded { border-left-color: var(--faso-warning); }
    .svc--down     { border-left-color: var(--faso-danger); }
    .svc header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin: 0 0 var(--faso-space-3);
    }
    .svc .status {
      display: inline-flex;
      align-items: center;
      gap: 4px;
      font-size: var(--faso-text-xs);
      font-weight: var(--faso-weight-semibold);
      color: var(--faso-text-muted);
    }
    .svc .status-dot {
      width: 8px; height: 8px;
      border-radius: 50%;
      background: var(--faso-text-subtle);
    }
    .svc--up       .status-dot { background: var(--faso-success); }
    .svc--degraded .status-dot { background: var(--faso-warning); }
    .svc--down     .status-dot { background: var(--faso-danger); }
    .metrics {
      list-style: none;
      padding: 0;
      margin: 0;
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 8px 16px;
    }
    .metrics li { display: flex; justify-content: space-between; font-size: var(--faso-text-sm); }
    .metrics li span { color: var(--faso-text-muted); }
    .svc footer {
      display: flex;
      justify-content: space-between;
      margin-top: var(--faso-space-3);
      padding-top: var(--faso-space-2);
      border-top: 1px solid var(--faso-border);
      font-size: var(--faso-text-xs);
      color: var(--faso-text-subtle);
    }
    .tag {
      text-transform: uppercase;
      letter-spacing: 0.06em;
    }

    .empty {
      padding: var(--faso-space-8);
      text-align: center;
      color: var(--faso-text-muted);
      background: var(--faso-surface);
      border: 1px dashed var(--faso-border-strong);
      border-radius: var(--faso-radius-xl);
    }

    .alerts {
      list-style: none;
      padding: 0;
      margin: 0;
      display: flex;
      flex-direction: column;
      gap: var(--faso-space-2);
    }
    .alerts li {
      display: grid;
      grid-template-columns: auto 1fr auto;
      gap: var(--faso-space-3);
      align-items: flex-start;
      padding: var(--faso-space-3) var(--faso-space-4);
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-lg);
    }
    .alerts li.ack { opacity: 0.6; }
    .alerts li strong { display: block; }
    .alerts li p { margin: 4px 0; color: var(--faso-text-muted); }
    .alerts li time { color: var(--faso-text-subtle); font-size: var(--faso-text-xs); }
    .alerts li .ack-by { color: var(--faso-text-subtle); font-size: var(--faso-text-xs); margin-left: 4px; }
    .alert--critical mat-icon { color: var(--faso-danger); }
    .alert--warning  mat-icon { color: var(--faso-warning); }
    .alert--info     mat-icon { color: var(--faso-info); }
  `],
})
export class AdminMonitoringComponent implements OnInit {
  readonly health = inject(PlatformHealthStore);

  readonly statusKind = computed(() => {
    const s = this.health.overallStatus();
    return s === 'HEALTHY' ? 'healthy' : s === 'DEGRADED' ? 'degraded' : 'critical';
  });

  ngOnInit(): void {
    if (this.health.services().length === 0) {
      this.health.loadMockSnapshot();
    }
  }
}
