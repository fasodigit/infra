// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, computed, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';
import { SectionHeaderComponent } from '@shared/components/section-header/section-header.component';
import { TrustBadgeComponent } from '@shared/components/trust-badge/trust-badge.component';

interface ChecklistStep {
  id: string;
  label: string;
  description: string;
  icon: string;
  done: boolean;
}

@Component({
  selector: 'app-halal-checklist',
  standalone: true,
  imports: [CommonModule, MatIconModule, MatButtonModule, SectionHeaderComponent, TrustBadgeComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="page">
      <div class="container">
        <header class="head">
          <div>
            <h1>Certification halal · Lot {{ lotId() }}</h1>
            <p>Complétez les étapes pour obtenir le badge halal.</p>
          </div>
          <app-trust-badge kind="halal" label="Halal certifié" />
        </header>

        <div class="progress-card">
          <div class="bar" role="progressbar"
               [attr.aria-valuenow]="progress()" aria-valuemin="0" aria-valuemax="100">
            <div class="fill" [style.width.%]="progress()"></div>
          </div>
          <div class="meter">
            <strong>{{ progress() }}%</strong>
            <span>{{ doneCount() }} / {{ steps().length }} étapes complétées</span>
          </div>
        </div>

        <app-section-header title="Étapes de certification" />

        <ol class="steps">
          @for (s of steps(); track s.id; let i = $index) {
            <li [class.done]="s.done">
              <span class="num">{{ i + 1 }}</span>
              <span class="dot"><mat-icon>{{ s.done ? 'check_circle' : s.icon }}</mat-icon></span>
              <div class="body">
                <strong>{{ s.label }}</strong>
                <p>{{ s.description }}</p>
                @if (!s.done) {
                  <button mat-button color="primary" type="button" (click)="complete(s.id)">
                    Marquer comme fait
                  </button>
                } @else {
                  <span class="state"><mat-icon>check</mat-icon> Validé</span>
                }
              </div>
            </li>
          }
        </ol>

        @if (progress() === 100) {
          <div class="success">
            <mat-icon>verified</mat-icon>
            <div>
              <h3>Félicitations, votre lot est certifié halal&nbsp;!</h3>
              <p>Le badge "Halal certifié" sera visible sur toutes vos annonces liées à ce lot.</p>
            </div>
          </div>
        }
      </div>
    </section>
  `,
  styles: [`
    :host { display: block; background: var(--faso-bg); min-height: 100vh; }
    .container {
      max-width: 900px;
      margin: 0 auto;
      padding: var(--faso-space-6) var(--faso-space-4) var(--faso-space-12);
    }
    .head {
      display: flex;
      justify-content: space-between;
      align-items: flex-start;
      gap: var(--faso-space-3);
      margin-bottom: var(--faso-space-6);
      flex-wrap: wrap;
    }
    .head h1 { margin: 0; font-size: var(--faso-text-2xl); font-weight: var(--faso-weight-bold); }
    .head p { margin: 4px 0 0; color: var(--faso-text-muted); }

    .progress-card {
      display: flex;
      align-items: center;
      gap: var(--faso-space-4);
      padding: var(--faso-space-5);
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
      margin-bottom: var(--faso-space-6);
    }
    .bar {
      flex: 1;
      height: 10px;
      background: var(--faso-surface-alt);
      border-radius: var(--faso-radius-pill);
      overflow: hidden;
    }
    .fill {
      height: 100%;
      background: linear-gradient(90deg, var(--faso-success), var(--faso-accent-500));
      border-radius: inherit;
      transition: width var(--faso-duration-slow) var(--faso-ease-standard);
    }
    .meter { text-align: right; white-space: nowrap; }
    .meter strong {
      display: block;
      font-size: var(--faso-text-xl);
      color: var(--faso-primary-700);
    }
    .meter span {
      color: var(--faso-text-muted);
      font-size: var(--faso-text-sm);
    }

    .steps {
      list-style: none;
      padding: 0;
      margin: 0;
      display: flex;
      flex-direction: column;
      gap: var(--faso-space-3);
    }
    .steps li {
      display: grid;
      grid-template-columns: auto auto 1fr;
      align-items: flex-start;
      gap: var(--faso-space-3);
      padding: var(--faso-space-4);
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
      transition: border-color var(--faso-duration-fast) var(--faso-ease-standard);
    }
    .steps li.done {
      border-color: var(--faso-success);
      background: var(--faso-success-bg);
    }

    .num {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      width: 28px;
      height: 28px;
      background: var(--faso-surface-alt);
      border-radius: 50%;
      color: var(--faso-text-muted);
      font-weight: var(--faso-weight-bold);
      font-size: var(--faso-text-sm);
    }
    .steps li.done .num {
      background: var(--faso-success);
      color: #FFFFFF;
    }
    .dot {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      color: var(--faso-accent-700);
    }
    .steps li.done .dot { color: var(--faso-success); }
    .body { display: flex; flex-direction: column; gap: 4px; }
    .body strong { font-size: var(--faso-text-base); }
    .body p { margin: 0; color: var(--faso-text-muted); }
    .body button { align-self: flex-start; margin-top: 4px; }
    .state {
      display: inline-flex;
      align-items: center;
      gap: 4px;
      color: var(--faso-success);
      font-weight: var(--faso-weight-semibold);
      font-size: var(--faso-text-sm);
      margin-top: 4px;
    }

    .success {
      display: flex;
      gap: var(--faso-space-3);
      align-items: center;
      padding: var(--faso-space-5);
      margin-top: var(--faso-space-6);
      background: var(--faso-success-bg);
      border: 1px solid var(--faso-success);
      border-radius: var(--faso-radius-xl);
    }
    .success mat-icon {
      font-size: 48px; width: 48px; height: 48px;
      color: var(--faso-success);
      flex-shrink: 0;
    }
    .success h3 { margin: 0; color: var(--faso-primary-800); }
    .success p { margin: 4px 0 0; color: var(--faso-text-muted); }
  `],
})
export class HalalChecklistComponent {
  readonly lotId = signal('L-2026-041');

  readonly steps = signal<ChecklistStep[]>([
    { id: 's1', label: 'Élevage halal conforme',
      description: 'Alimentation sans farines animales, eau propre, traitements interdits par la charia évités.',
      icon: 'eco', done: true },
    { id: 's2', label: 'Identification du lot',
      description: 'Bague ou marquage de chaque sujet pour traçabilité individuelle.',
      icon: 'bookmarks', done: true },
    { id: 's3', label: 'Abattoir agréé halal',
      description: 'Réservation auprès d\'un abattoir certifié par le CERFI ou la Communauté musulmane du Burkina.',
      icon: 'store', done: false },
    { id: 's4', label: 'Présence du sacrificateur',
      description: 'Sacrificateur musulman agréé présent lors de l\'abattage (tasmiyah obligatoire).',
      icon: 'person', done: false },
    { id: 's5', label: 'Contrôle vétérinaire post-abattage',
      description: 'Vérification sanitaire systématique avant conditionnement.',
      icon: 'medical_services', done: false },
    { id: 's6', label: 'Émission du certificat',
      description: 'Certificat numérique signé + QR code généré automatiquement.',
      icon: 'verified', done: false },
  ]);

  readonly doneCount = computed(() => this.steps().filter(s => s.done).length);
  readonly progress = computed(() => Math.round(this.doneCount() / this.steps().length * 100));

  complete(id: string): void {
    this.steps.update(arr => arr.map(s => s.id === id ? { ...s, done: true } : s));
  }
}
