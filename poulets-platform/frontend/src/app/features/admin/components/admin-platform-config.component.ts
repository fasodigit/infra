// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';
import { MatSnackBar } from '@angular/material/snack-bar';
import { inject } from '@angular/core';

import { SectionHeaderComponent } from '@shared/components/section-header/section-header.component';
import { FeatureFlag } from '@shared/models/admin.models';

@Component({
  selector: 'app-admin-platform-config',
  standalone: true,
  imports: [CommonModule, FormsModule, MatIconModule, MatButtonModule, SectionHeaderComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="page">
      <header>
        <h1>Configuration plateforme</h1>
        <p>Feature flags, maintenance, notifications système</p>
      </header>

      <app-section-header title="Feature flags" kicker="Activer / désactiver" />
      <ul class="flags">
        @for (f of flags(); track f.key) {
          <li>
            <div>
              <strong>{{ f.label }}</strong>
              <span class="key">{{ f.key }}</span>
              @if (f.description) { <p>{{ f.description }}</p> }
              @if (f.rolloutPercent != null) {
                <p class="rollout">Rollout progressif&nbsp;: {{ f.rolloutPercent }}%</p>
              }
            </div>
            <label class="switch" [attr.aria-label]="f.label">
              <input
                type="checkbox"
                [checked]="f.enabled"
                (change)="toggleFlag(f.key, $event)"
              >
              <span class="track"><span class="thumb"></span></span>
            </label>
          </li>
        }
      </ul>

      <app-section-header title="Mode maintenance" kicker="Zone critique" />
      <div class="maintenance" [class.active]="maintenance()">
        <div>
          <strong>{{ maintenance() ? 'Maintenance en cours' : 'Plateforme active' }}</strong>
          <p>
            @if (maintenance()) {
              Les utilisateurs voient un écran de maintenance. Seuls les admins peuvent se connecter.
            } @else {
              Activer cette option bloquera l'accès aux utilisateurs non-admin.
            }
          </p>
          @if (maintenance()) {
            <textarea
              rows="2"
              placeholder="Message affiché aux utilisateurs…"
              [(ngModel)]="maintenanceMessage"
            ></textarea>
          }
        </div>
        <button
          mat-flat-button
          [color]="maintenance() ? 'primary' : 'warn'"
          type="button"
          (click)="toggleMaintenance()"
        >
          @if (maintenance()) {
            <mat-icon>play_arrow</mat-icon> Reprendre l'activité
          } @else {
            <mat-icon>power_settings_new</mat-icon> Activer le mode maintenance
          }
        </button>
      </div>

      <app-section-header title="Tests & opérations" />
      <div class="ops">
        <button mat-stroked-button type="button" (click)="testEmail()">
          <mat-icon>outgoing_mail</mat-icon>
          Envoyer un email test
        </button>
        <button mat-stroked-button type="button" (click)="clearCache()">
          <mat-icon>delete_sweep</mat-icon>
          Vider le cache KAYA
        </button>
        <button mat-stroked-button type="button" (click)="refetchFlags()">
          <mat-icon>refresh</mat-icon>
          Recharger les feature flags
        </button>
      </div>
    </section>
  `,
  styles: [`
    :host { display: block; }
    header { margin-bottom: var(--faso-space-6); }
    header h1 { margin: 0; font-size: var(--faso-text-3xl); font-weight: var(--faso-weight-bold); }
    header p { margin: 4px 0 0; color: var(--faso-text-muted); }

    .flags {
      list-style: none;
      padding: 0;
      margin: 0 0 var(--faso-space-8);
      display: flex;
      flex-direction: column;
      gap: var(--faso-space-2);
    }
    .flags li {
      display: flex;
      justify-content: space-between;
      align-items: flex-start;
      gap: var(--faso-space-4);
      padding: var(--faso-space-4);
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-lg);
    }
    .flags strong { display: block; }
    .flags .key {
      display: inline-block;
      margin-top: 2px;
      padding: 2px 6px;
      background: var(--faso-surface-alt);
      border-radius: var(--faso-radius-sm);
      font-family: var(--faso-font-mono);
      font-size: var(--faso-text-xs);
      color: var(--faso-text-muted);
    }
    .flags p { margin: 6px 0 0; color: var(--faso-text-muted); font-size: var(--faso-text-sm); }
    .flags .rollout { color: var(--faso-accent-700); font-weight: var(--faso-weight-medium); }

    .switch { position: relative; cursor: pointer; flex-shrink: 0; }
    .switch input {
      position: absolute;
      opacity: 0;
      width: 0;
      height: 0;
    }
    .switch .track {
      display: inline-block;
      width: 44px;
      height: 24px;
      background: var(--faso-border-strong);
      border-radius: var(--faso-radius-pill);
      position: relative;
      transition: background var(--faso-duration-fast) var(--faso-ease-standard);
    }
    .switch .thumb {
      position: absolute;
      top: 2px;
      left: 2px;
      width: 20px;
      height: 20px;
      background: #FFFFFF;
      border-radius: 50%;
      transition: left var(--faso-duration-fast) var(--faso-ease-standard);
    }
    .switch input:checked + .track {
      background: var(--faso-primary-600);
    }
    .switch input:checked + .track .thumb {
      left: 22px;
    }
    .switch input:focus-visible + .track {
      box-shadow: 0 0 0 3px var(--faso-primary-100);
    }

    .maintenance {
      display: flex;
      gap: var(--faso-space-4);
      align-items: flex-start;
      padding: var(--faso-space-5);
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
      margin-bottom: var(--faso-space-8);
    }
    .maintenance.active {
      border-color: var(--faso-warning);
      background: var(--faso-warning-bg);
    }
    .maintenance > div { flex: 1; }
    .maintenance strong { display: block; font-size: var(--faso-text-lg); }
    .maintenance p { margin: 4px 0; color: var(--faso-text-muted); }
    .maintenance textarea {
      width: 100%;
      padding: 8px 12px;
      border: 1px solid var(--faso-border-strong);
      border-radius: var(--faso-radius-md);
      font-family: inherit;
      font-size: var(--faso-text-sm);
      margin-top: 8px;
      resize: vertical;
    }

    .ops {
      display: flex;
      gap: var(--faso-space-2);
      flex-wrap: wrap;
    }
  `],
})
export class AdminPlatformConfigComponent {
  private readonly snack = inject(MatSnackBar);

  readonly flags = signal<FeatureFlag[]>([
    { key: 'marketplace.search.ai',    label: 'Recherche IA',                 description: 'Matching sémantique annonces/besoins.',    enabled: true,  rolloutPercent: 100 },
    { key: 'mfa.passkey',              label: 'PassKey obligatoire',          description: 'Impose la configuration PassKey à la 1ère connexion.', enabled: false, rolloutPercent: 25 },
    { key: 'orders.mobile_money',      label: 'Paiement Orange/Moov Money',   description: 'Activer le paiement mobile money réel.',   enabled: false },
    { key: 'notifications.sms',        label: 'Notifications SMS',            description: 'Envoi SMS via provider externe.',          enabled: false },
    { key: 'admin.audit.export_csv',   label: 'Export CSV logs audit',        description: 'Autoriser l\'export des logs en CSV.',     enabled: true },
  ]);

  readonly maintenance = signal(false);
  maintenanceMessage = 'Maintenance programmée — retour dans 15 minutes.';

  toggleFlag(key: string, ev: Event): void {
    const enabled = (ev.target as HTMLInputElement).checked;
    this.flags.update((arr) => arr.map((f) => f.key === key ? { ...f, enabled, updatedAt: new Date().toISOString() } : f));
    this.snack.open(`Flag ${key} ${enabled ? 'activé' : 'désactivé'}`, 'OK', { duration: 2500 });
  }

  toggleMaintenance(): void {
    this.maintenance.update((m) => !m);
    this.snack.open(this.maintenance() ? 'Mode maintenance activé' : 'Mode maintenance désactivé', 'OK', { duration: 2500 });
  }

  testEmail(): void {
    this.snack.open('Email test envoyé (stub)', 'OK', { duration: 2500 });
  }
  clearCache(): void {
    this.snack.open('Cache KAYA vidé (stub)', 'OK', { duration: 2500 });
  }
  refetchFlags(): void {
    this.snack.open('Feature flags rechargés (stub)', 'OK', { duration: 2500 });
  }
}
