// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { CommonModule } from '@angular/common';
import {
  ChangeDetectionStrategy,
  Component,
  computed,
  input,
  signal,
} from '@angular/core';
import { TranslateModule } from '@ngx-translate/core';
import {
  FasoAvatarComponent,
  FasoIconComponent,
  FasoRoleChipComponent,
} from '../components-v2';
import { AdminLang, AuditEntry } from '../models/admin.model';
import {
  MOCK_AUDIT,
  MOCK_CHART,
  MOCK_SERVICES,
  MOCK_USERS,
} from '../services/admin-mocks';

interface ChartGeometry {
  readonly width: number;
  readonly height: number;
  readonly padding: number;
  readonly points: readonly [number, number][];
  readonly path: string;
  readonly area: string;
  readonly gridLines: readonly number[];
  readonly labels: readonly { x: number; y: number; text: string }[];
}

@Component({
  selector: 'faso-dashboard-v2-page',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [
    CommonModule,
    TranslateModule,
    FasoIconComponent,
    FasoAvatarComponent,
  ],
  template: `
    @if (breakGlass()) {
      <div class="fd-banner danger">
        <faso-icon name="flame" [size]="18" />
        <div class="fd-banner-body">
          <strong>Break-Glass actif · Ibrahim Compaoré</strong>
          <small>
            {{
              lang() === 'fr'
                ? 'Élévation temporaire SUPER-ADMIN — expire dans 3h 42min · justification : « Incident SEV-1 base de données état-civil ». Tous les SUPER-ADMIN ont été notifiés.'
                : 'Temporary SUPER-ADMIN elevation — expires in 3h 42min · justification: "SEV-1 incident on civil-registry database". All SUPER-ADMINs have been notified.'
            }}
          </small>
        </div>
        <button class="fd-btn sm">
          {{ lang() === 'fr' ? 'Voir détails' : 'View details' }}
        </button>
        <button class="fd-btn sm danger">
          {{ lang() === 'fr' ? 'Révoquer' : 'Revoke' }}
        </button>
      </div>
    }

    <div class="fd-page-head">
      <div>
        <div class="fd-h1">
          {{ lang() === 'fr' ? 'Tableau de bord' : 'Dashboard' }}
        </div>
        <div class="fd-page-sub">
          {{
            lang() === 'fr'
              ? 'Vue consolidée · sécurité, identité et conformité de la plateforme souveraine.'
              : 'Consolidated view · security, identity, and compliance of the sovereign platform.'
          }}
        </div>
      </div>
      <div class="fd-row">
        <span class="fd-help">
          {{ lang() === 'fr' ? 'Dernière sync' : 'Last sync' }} ·
          <span class="fd-mono">10:42:18</span> Africa/Ouagadougou
        </span>
        <button class="fd-btn sm">
          <faso-icon name="refresh" [size]="13" />
          {{ lang() === 'fr' ? 'Actualiser' : 'Refresh' }}
        </button>
      </div>
    </div>

    <div class="fd-kpi-grid">
      <div class="fd-kpi">
        <div class="fd-kpi-label">
          {{ 'admin.kpi.activeUsers' | translate }}
        </div>
        <div class="fd-kpi-value">2 184</div>
        <div class="fd-kpi-delta up">
          <faso-icon name="arrowUp" [size]="12" /> +12,4% vs s-1
        </div>
      </div>
      <div class="fd-kpi">
        <div class="fd-kpi-label">{{ 'admin.kpi.otpSent' | translate }}</div>
        <div class="fd-kpi-value">421</div>
        <div class="fd-kpi-delta up">
          <faso-icon name="arrowUp" [size]="12" /> +8,1% vs h-1
        </div>
      </div>
      <div class="fd-kpi">
        <div class="fd-kpi-label">{{ 'admin.kpi.sessions' | translate }}</div>
        <div class="fd-kpi-value">58</div>
        <div class="fd-kpi-delta down">
          <faso-icon name="arrowDown" [size]="12" /> -3 depuis 09:00
        </div>
      </div>
      <div class="fd-kpi" style="border-color: rgba(198,40,40,0.3);">
        <div class="fd-kpi-label" style="color: var(--danger);">
          {{ 'admin.kpi.alerts' | translate }}
        </div>
        <div class="fd-kpi-value" style="color: var(--danger);">3</div>
        <div class="fd-kpi-delta" style="color: var(--text-3);">
          {{
            lang() === 'fr'
              ? '2 critiques · 1 avertissement'
              : '2 critical · 1 warning'
          }}
        </div>
      </div>
    </div>

    <div
      style="display: grid; grid-template-columns: 2fr 1fr; gap: 16px; margin-bottom: 16px;"
    >
      <div class="fd-card">
        <div class="fd-card-h">
          <div class="fd-card-h-title">
            {{ lang() === 'fr' ? 'Activité 7 jours' : '7-day activity' }}
          </div>
          <div class="fd-row" style="gap: 14px;">
            <span
              class="fd-row"
              style="font-size: 12px; color: var(--text-3);"
            >
              <span class="fd-dot ok"></span> OTP
              {{ lang() === 'fr' ? 'émis' : 'issued' }}
            </span>
            <span
              class="fd-row"
              style="font-size: 12px; color: var(--text-3);"
            >
              <span class="fd-dot" style="background: var(--accent);"></span>
              {{ lang() === 'fr' ? 'Sessions' : 'Sessions' }}
            </span>
          </div>
        </div>
        <div class="fd-card-b">
          <svg
            [attr.viewBox]="
              '0 0 ' + chart().width + ' ' + chart().height
            "
            class="fd-chart"
            preserveAspectRatio="none"
          >
            <defs>
              <linearGradient id="grad1" x1="0" y1="0" x2="0" y2="1">
                <stop
                  offset="0%"
                  stop-color="var(--primary)"
                  stop-opacity="0.25"
                />
                <stop
                  offset="100%"
                  stop-color="var(--primary)"
                  stop-opacity="0"
                />
              </linearGradient>
            </defs>
            @for (gy of chart().gridLines; track gy) {
              <line
                [attr.x1]="chart().padding"
                [attr.x2]="chart().width - chart().padding"
                [attr.y1]="gy"
                [attr.y2]="gy"
                stroke="var(--border)"
                stroke-dasharray="2 4"
              />
            }
            <path [attr.d]="chart().area" fill="url(#grad1)" />
            <path
              [attr.d]="chart().path"
              stroke="var(--primary)"
              stroke-width="2"
              fill="none"
            />
            @for (p of chart().points; track $index) {
              <circle
                [attr.cx]="p[0]"
                [attr.cy]="p[1]"
                r="3.5"
                fill="var(--surface)"
                stroke="var(--primary)"
                stroke-width="2"
              />
            }
            @for (lbl of chart().labels; track lbl.text) {
              <text
                [attr.x]="lbl.x"
                [attr.y]="lbl.y"
                font-size="11"
                fill="var(--text-3)"
                text-anchor="middle"
              >
                {{ lbl.text }}
              </text>
            }
          </svg>
        </div>
      </div>

      <div class="fd-card">
        <div class="fd-card-h">
          <div class="fd-card-h-title">
            {{ lang() === 'fr' ? 'Santé services' : 'Service health' }}
          </div>
          <span class="fd-chip muted" style="font-size: 11px;">auto · 30s</span>
        </div>
        <div
          class="fd-card-b"
          style="display: flex; flex-direction: column; gap: 8px;"
        >
          @for (s of services(); track s.name) {
            <div class="fd-health">
              <span
                class="fd-dot"
                [class.ok]="s.status === 'ok'"
                [class.warn]="s.status === 'warn'"
                [class.danger]="s.status === 'down'"
              ></span>
              <div style="flex: 1; min-width: 0;">
                <div class="fd-health-name">
                  {{ s.name }}
                  <span
                    class="fd-mono"
                    style="color: var(--text-3); font-size: 11px; font-weight: 400;"
                  >
                    {{ s.port }}
                  </span>
                </div>
                <div class="fd-health-meta">{{ s.meta }}</div>
              </div>
            </div>
          }
        </div>
      </div>
    </div>

    <div class="fd-card">
      <div class="fd-card-h">
        <div class="fd-card-h-title">
          {{ lang() === 'fr' ? 'Audit récent' : 'Recent audit' }}
        </div>
        <button class="fd-btn ghost sm">
          {{ lang() === 'fr' ? 'Voir tout' : 'View all' }}
          <faso-icon name="chevR" [size]="12" />
        </button>
      </div>
      <div class="fd-card-b" style="padding: 0;">
        <table class="fd-table">
          <thead>
            <tr>
              <th>{{ lang() === 'fr' ? 'Action' : 'Action' }}</th>
              <th>{{ lang() === 'fr' ? 'Acteur' : 'Actor' }}</th>
              <th>{{ lang() === 'fr' ? 'Détail' : 'Detail' }}</th>
              <th>{{ lang() === 'fr' ? 'Trace' : 'Trace' }}</th>
              <th style="text-align: right;">
                {{ lang() === 'fr' ? 'Heure' : 'Time' }}
              </th>
            </tr>
          </thead>
          <tbody>
            @for (a of recentAudit(); track a.id) {
              <tr>
                <td>
                  <span
                    class="fd-chip fd-mono"
                    [class.danger]="
                      a.action === 'BREAK_GLASS_ACTIVATED' ||
                      a.action === 'OTP_FAILED'
                    "
                    [class.ok]="
                      a.action === 'ROLE_GRANTED' ||
                      a.action === 'MFA_ENROLLED'
                    "
                    [class.warn]="a.action === 'SETTINGS_UPDATED'"
                    [class.info]="
                      a.action !== 'BREAK_GLASS_ACTIVATED' &&
                      a.action !== 'OTP_FAILED' &&
                      a.action !== 'ROLE_GRANTED' &&
                      a.action !== 'MFA_ENROLLED' &&
                      a.action !== 'SETTINGS_UPDATED'
                    "
                    style="font-size: 11px;"
                  >
                    {{ a.action }}
                  </span>
                </td>
                <td>
                  @if (actorOf(a); as actor) {
                    <div class="fd-user-cell">
                      <faso-avatar [user]="actor" [size]="26" />
                      <div>
                        <div style="font-size: 12.5px; font-weight: 500;">
                          {{ actor.firstName }} {{ actor.lastName }}
                        </div>
                        <div style="font-size: 11px; color: var(--text-3);">
                          {{ actor.role }}
                        </div>
                      </div>
                    </div>
                  }
                </td>
                <td style="color: var(--text-2); font-size: 12.5px;">
                  {{ a.desc }}
                </td>
                <td>
                  <span class="fd-mono-pill">{{ a.traceId }}</span>
                </td>
                <td
                  class="fd-mono"
                  style="text-align: right; color: var(--text-3); font-size: 12px;"
                >
                  {{ a.time }}
                </td>
              </tr>
            }
          </tbody>
        </table>
      </div>
    </div>
  `,
  styles: [
    `
      :host {
        display: contents;
      }
    `,
  ],
})
export class DashboardV2Page {
  readonly lang = input<AdminLang>('fr');
  readonly breakGlass = input<boolean>(false);

  protected readonly users = signal(MOCK_USERS);
  protected readonly audit = signal(MOCK_AUDIT);
  protected readonly chartData = signal(MOCK_CHART);
  protected readonly services = signal(MOCK_SERVICES);

  protected readonly recentAudit = computed<readonly AuditEntry[]>(() =>
    this.audit().slice(0, 6),
  );

  protected readonly chart = computed<ChartGeometry>(() => {
    const data = this.chartData();
    const W = 720;
    const H = 200;
    const P = 24;
    const max = Math.max(...data.map((d) => d.otp));
    const xStep = (W - 2 * P) / (data.length - 1);

    const points = data.map(
      (d, i) =>
        [P + i * xStep, H - P - (d.otp / max) * (H - 2 * P)] as [
          number,
          number,
        ],
    );
    const path = points
      .map(
        (p, i) =>
          (i ? 'L' : 'M') + p[0].toFixed(1) + ',' + p[1].toFixed(1),
      )
      .join(' ');
    const last = points[points.length - 1];
    const first = points[0];
    const area = `${path} L${last[0]},${H - P} L${first[0]},${H - P} Z`;

    const gridLines = [0, 1, 2, 3].map((i) => P + i * ((H - 2 * P) / 3));
    const labels = data.map((d, i) => ({
      x: points[i][0],
      y: H - 6,
      text: d.d,
    }));

    return { width: W, height: H, padding: P, points, path, area, gridLines, labels };
  });

  protected actorOf(entry: AuditEntry) {
    return this.users().find((u) => u.id === entry.actor);
  }
}
