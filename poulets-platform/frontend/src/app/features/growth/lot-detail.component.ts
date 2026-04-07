import { Component, OnInit, signal, computed } from '@angular/core';
import { CommonModule, DatePipe, DecimalPipe } from '@angular/common';
import { ActivatedRoute, RouterLink } from '@angular/router';
import { MatCardModule } from '@angular/material/card';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatTableModule } from '@angular/material/table';
import { MatTabsModule } from '@angular/material/tabs';
import { MatDividerModule } from '@angular/material/divider';
import { TranslateModule } from '@ngx-translate/core';
import { StatusBadgeComponent } from '../../shared/components/status-badge/status-badge.component';
import { Lot, MesureCroissance, Race } from '../../shared/models/poulet.model';
import { PlanAlimentaire, PhaseAlimentaire } from '../../shared/models/aliment.model';

@Component({
  selector: 'app-lot-detail',
  standalone: true,
  imports: [
    CommonModule,
    RouterLink,
    MatCardModule,
    MatButtonModule,
    MatIconModule,
    MatTableModule,
    MatTabsModule,
    MatDividerModule,
    TranslateModule,
    StatusBadgeComponent,
    DatePipe,
    DecimalPipe,
  ],
  template: `
    <div class="lot-detail-container">
      <div class="page-header">
        <button mat-icon-button routerLink="..">
          <mat-icon>arrow_back</mat-icon>
        </button>
        <div>
          <h1>{{ lot()?.nom }}</h1>
          @if (lot(); as l) {
            <app-status-badge [status]="l.statut"></app-status-badge>
          }
        </div>
        <span class="spacer"></span>
        <a mat-raised-button color="primary" [routerLink]="['add-weight']">
          <mat-icon>add</mat-icon>
          {{ 'growth.detail.add_weight' | translate }}
        </a>
      </div>

      @if (lot(); as l) {
        <!-- Lot Info Cards -->
        <div class="info-grid">
          <mat-card class="info-card">
            <mat-card-content>
              <span class="info-label">{{ 'growth.detail.race' | translate }}</span>
              <span class="info-value">{{ l.race }}</span>
            </mat-card-content>
          </mat-card>
          <mat-card class="info-card">
            <mat-card-content>
              <span class="info-label">{{ 'growth.detail.count' | translate }}</span>
              <span class="info-value">{{ l.effectifActuel }} / {{ l.effectifInitial }}</span>
            </mat-card-content>
          </mat-card>
          <mat-card class="info-card">
            <mat-card-content>
              <span class="info-label">{{ 'growth.detail.avg_weight' | translate }}</span>
              <span class="info-value">{{ l.poidsMoyen | number:'1.2-2' }} kg</span>
            </mat-card-content>
          </mat-card>
          <mat-card class="info-card">
            <mat-card-content>
              <span class="info-label">{{ 'growth.detail.mortality' | translate }}</span>
              <span class="info-value">{{ l.tauxMortalite | number:'1.1-1' }}%</span>
            </mat-card-content>
          </mat-card>
        </div>

        <mat-tab-group>
          <!-- Growth Curve Tab -->
          <mat-tab label="{{ 'growth.detail.tab_curve' | translate }}">
            <div class="tab-content">
              <mat-card>
                <mat-card-header>
                  <mat-card-title>{{ 'growth.detail.growth_curve' | translate }}</mat-card-title>
                </mat-card-header>
                <mat-card-content>
                  <div class="chart-container">
                    <svg viewBox="0 0 500 280" class="growth-chart">
                      <!-- Y axis labels -->
                      @for (label of yLabels(); track label.y) {
                        <text [attr.x]="35" [attr.y]="label.y + 4" font-size="10" fill="#999"
                              text-anchor="end">{{ label.text }}</text>
                        <line [attr.x1]="40" [attr.y1]="label.y" [attr.x2]="480" [attr.y2]="label.y"
                              stroke="#f0f0f0" stroke-width="1"/>
                      }

                      <!-- Actual weight line -->
                      <polyline [attr.points]="actualPoints()" fill="none"
                                stroke="#4caf50" stroke-width="2.5" stroke-linejoin="round"/>
                      <!-- Dots on actual line -->
                      @for (pt of actualDots(); track pt.x) {
                        <circle [attr.cx]="pt.x" [attr.cy]="pt.y" r="4" fill="#4caf50"/>
                      }

                      <!-- Target line -->
                      <polyline [attr.points]="targetPoints()" fill="none"
                                stroke="#ff9800" stroke-width="2" stroke-dasharray="6,3"/>

                      <!-- Legend -->
                      <rect x="60" y="15" width="14" height="4" fill="#4caf50" rx="2"/>
                      <text x="80" y="19" font-size="10" fill="#666">
                        {{ 'growth.detail.actual_weight' | translate }}
                      </text>
                      <rect x="200" y="15" width="14" height="4" fill="#ff9800" rx="2"/>
                      <text x="220" y="19" font-size="10" fill="#666">
                        {{ 'growth.detail.target_weight' | translate }}
                      </text>

                      <!-- X axis labels -->
                      @for (label of xLabels(); track label.x) {
                        <text [attr.x]="label.x" y="275" font-size="10" fill="#999"
                              text-anchor="middle">{{ label.text }}</text>
                      }
                    </svg>
                  </div>
                </mat-card-content>
              </mat-card>
            </div>
          </mat-tab>

          <!-- Feed Plan Tab -->
          <mat-tab label="{{ 'growth.detail.tab_feed' | translate }}">
            <div class="tab-content">
              <mat-card>
                <mat-card-header>
                  <mat-card-title>{{ 'growth.detail.feed_plan' | translate }}</mat-card-title>
                </mat-card-header>
                <mat-card-content>
                  <div class="feed-phases">
                    @for (phase of feedPlan()?.phases || []; track phase.semaineDe) {
                      <div class="feed-phase" [class]="'phase-' + getPhaseType(phase)">
                        <div class="phase-header">
                          <mat-icon>{{ getPhaseIcon(phase) }}</mat-icon>
                          <span class="phase-name">{{ phase.alimentNom }}</span>
                        </div>
                        <div class="phase-details">
                          <span>{{ 'growth.detail.weeks' | translate }}
                            {{ phase.semaineDe }} - {{ phase.semaineA }}</span>
                          <span>{{ phase.quantiteJournaliereParTete }}g / {{ 'growth.detail.day_bird' | translate }}</span>
                        </div>
                      </div>
                    }
                  </div>
                </mat-card-content>
              </mat-card>
            </div>
          </mat-tab>

          <!-- Weight Log Tab -->
          <mat-tab label="{{ 'growth.detail.tab_log' | translate }}">
            <div class="tab-content">
              <mat-card>
                <mat-card-header>
                  <mat-card-title>{{ 'growth.detail.weight_log' | translate }}</mat-card-title>
                </mat-card-header>
                <mat-card-content>
                  @if (mesures().length > 0) {
                    <table mat-table [dataSource]="mesures()" class="full-width-table">
                      <ng-container matColumnDef="date">
                        <th mat-header-cell *matHeaderCellDef>{{ 'growth.detail.date' | translate }}</th>
                        <td mat-cell *matCellDef="let m">{{ m.date | date:'dd/MM/yyyy' }}</td>
                      </ng-container>
                      <ng-container matColumnDef="poidsMoyen">
                        <th mat-header-cell *matHeaderCellDef>{{ 'growth.detail.avg_weight' | translate }}</th>
                        <td mat-cell *matCellDef="let m">{{ m.poidsMoyen | number:'1.2-2' }} kg</td>
                      </ng-container>
                      <ng-container matColumnDef="effectif">
                        <th mat-header-cell *matHeaderCellDef>{{ 'growth.detail.count' | translate }}</th>
                        <td mat-cell *matCellDef="let m">{{ m.effectif }}</td>
                      </ng-container>
                      <ng-container matColumnDef="alimentConsomme">
                        <th mat-header-cell *matHeaderCellDef>{{ 'growth.detail.feed_consumed' | translate }}</th>
                        <td mat-cell *matCellDef="let m">
                          {{ m.alimentConsomme ? (m.alimentConsomme | number:'1.0-0') + ' kg' : '-' }}
                        </td>
                      </ng-container>
                      <ng-container matColumnDef="observations">
                        <th mat-header-cell *matHeaderCellDef>{{ 'growth.detail.notes' | translate }}</th>
                        <td mat-cell *matCellDef="let m">{{ m.observations || '-' }}</td>
                      </ng-container>
                      <tr mat-header-row *matHeaderRowDef="logColumns"></tr>
                      <tr mat-row *matRowDef="let row; columns: logColumns;"></tr>
                    </table>
                  } @else {
                    <div class="empty-state">
                      <p>{{ 'growth.detail.no_measures' | translate }}</p>
                    </div>
                  }
                </mat-card-content>
              </mat-card>
            </div>
          </mat-tab>
        </mat-tab-group>
      }
    </div>
  `,
  styles: [`
    .lot-detail-container {
      padding: 24px;
      max-width: 1100px;
      margin: 0 auto;
    }

    .page-header {
      display: flex;
      align-items: center;
      gap: 12px;
      margin-bottom: 24px;

      h1 { margin: 0; color: var(--faso-primary-dark, #1b5e20); }
      .spacer { flex: 1; }
    }

    .info-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
      gap: 16px;
      margin-bottom: 24px;
    }

    .info-card mat-card-content {
      display: flex;
      flex-direction: column;
      align-items: center;
      text-align: center;

      .info-label { font-size: 0.8rem; color: #666; margin-bottom: 4px; }
      .info-value { font-size: 1.3rem; font-weight: 700; }
    }

    .tab-content { padding: 16px 0; }

    .chart-container { width: 100%; overflow-x: auto; }

    .growth-chart { width: 100%; max-height: 300px; }

    .feed-phases {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
      gap: 16px;
      padding: 16px 0;
    }

    .feed-phase {
      padding: 16px;
      border-radius: 12px;
      border: 2px solid #e0e0e0;

      &.phase-demarrage { border-color: #ffeb3b; background: #fffde7; }
      &.phase-croissance { border-color: #4caf50; background: #e8f5e9; }
      &.phase-finition { border-color: #ff9800; background: #fff3e0; }

      .phase-header {
        display: flex;
        align-items: center;
        gap: 8px;
        margin-bottom: 8px;
        font-weight: 600;
      }

      .phase-details {
        display: flex;
        flex-direction: column;
        gap: 4px;
        font-size: 0.85rem;
        color: #666;
      }
    }

    .full-width-table { width: 100%; }

    .empty-state {
      text-align: center;
      padding: 32px;
      color: #999;
    }
  `],
})
export class LotDetailComponent implements OnInit {
  readonly lot = signal<Lot | null>(null);
  readonly mesures = signal<MesureCroissance[]>([]);
  readonly feedPlan = signal<PlanAlimentaire | null>(null);

  readonly actualPoints = signal('');
  readonly targetPoints = signal('');
  readonly actualDots = signal<{ x: number; y: number }[]>([]);
  readonly yLabels = signal<{ y: number; text: string }[]>([]);
  readonly xLabels = signal<{ x: number; text: string }[]>([]);

  readonly logColumns = ['date', 'poidsMoyen', 'effectif', 'alimentConsomme', 'observations'];

  constructor(private readonly route: ActivatedRoute) {}

  ngOnInit(): void {
    const lotId = this.route.snapshot.paramMap.get('lotId');
    this.loadLot(lotId!);
  }

  getPhaseType(phase: PhaseAlimentaire): string {
    if (phase.semaineDe <= 3) return 'demarrage';
    if (phase.semaineDe <= 6) return 'croissance';
    return 'finition';
  }

  getPhaseIcon(phase: PhaseAlimentaire): string {
    if (phase.semaineDe <= 3) return 'egg';
    if (phase.semaineDe <= 6) return 'trending_up';
    return 'emoji_events';
  }

  private loadLot(lotId: string): void {
    const measures: MesureCroissance[] = [
      { id: 'm1', lotId, date: '2026-02-22', poidsMoyen: 0.18, effectif: 200, alimentConsomme: 25 },
      { id: 'm2', lotId, date: '2026-03-01', poidsMoyen: 0.35, effectif: 199, alimentConsomme: 50 },
      { id: 'm3', lotId, date: '2026-03-08', poidsMoyen: 0.60, effectif: 198, alimentConsomme: 85 },
      { id: 'm4', lotId, date: '2026-03-15', poidsMoyen: 0.90, effectif: 197, alimentConsomme: 120 },
      { id: 'm5', lotId, date: '2026-03-22', poidsMoyen: 1.20, effectif: 196, alimentConsomme: 160 },
      { id: 'm6', lotId, date: '2026-03-29', poidsMoyen: 1.50, effectif: 196, alimentConsomme: 195 },
      { id: 'm7', lotId, date: '2026-04-05', poidsMoyen: 1.85, effectif: 195, alimentConsomme: 220, observations: 'Bonne croissance' },
    ];

    this.lot.set({
      id: lotId, nom: 'Lot A - Brahma', race: Race.BRAHMA,
      effectifInitial: 200, effectifActuel: 195, dateArrivee: '2026-02-15',
      ageArrivee: 1, poidsArrivee: 0.12, poidsMoyen: 1.85,
      tauxMortalite: 2.5, indiceConversion: 1.8,
      statut: 'EN_COURS', mesures: measures, eleveurId: 'e1', createdAt: '2026-02-15',
    });
    this.mesures.set(measures);

    this.feedPlan.set({
      id: 'fp1', lotId, coutEstime: 125000, createdAt: '2026-02-15',
      phases: [
        { semaineDe: 1, semaineA: 3, alimentNom: 'Demarrage', quantiteJournaliereParTete: 30 },
        { semaineDe: 4, semaineA: 6, alimentNom: 'Croissance', quantiteJournaliereParTete: 80 },
        { semaineDe: 7, semaineA: 9, alimentNom: 'Finition', quantiteJournaliereParTete: 120 },
      ],
    });

    this.buildChart(measures);
  }

  private buildChart(measures: MesureCroissance[]): void {
    const target = [0.15, 0.35, 0.65, 1.00, 1.35, 1.70, 2.00, 2.30, 2.50];
    const maxW = 3.0;
    const chartW = 420;
    const chartH = 220;
    const offsetX = 50;
    const offsetY = 35;

    const toXY = (values: number[]) =>
      values.map((v, i) => ({
        x: offsetX + (i / (Math.max(values.length, target.length) - 1)) * chartW,
        y: offsetY + chartH - (v / maxW) * chartH,
      }));

    const actualXY = toXY(measures.map(m => m.poidsMoyen));
    const targetXY = toXY(target);

    this.actualPoints.set(actualXY.map(p => `${p.x},${p.y}`).join(' '));
    this.targetPoints.set(targetXY.map(p => `${p.x},${p.y}`).join(' '));
    this.actualDots.set(actualXY);

    this.yLabels.set(
      [0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0].map(w => ({
        y: offsetY + chartH - (w / maxW) * chartH,
        text: w.toFixed(1),
      }))
    );

    this.xLabels.set(
      Array.from({ length: 9 }, (_, i) => ({
        x: offsetX + (i / 8) * chartW,
        text: `S${i + 1}`,
      }))
    );
  }
}
