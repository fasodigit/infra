// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { CommonModule } from '@angular/common';
import {
  ChangeDetectionStrategy,
  Component,
  DestroyRef,
  computed,
  inject,
  signal,
} from '@angular/core';
import { takeUntilDestroyed } from '@angular/core/rxjs-interop';
import { FormsModule } from '@angular/forms';
import {
  MAT_DIALOG_DATA,
  MatDialogModule,
  MatDialogRef,
} from '@angular/material/dialog';
import { TranslateModule } from '@ngx-translate/core';
import { interval } from 'rxjs';
import {
  FasoAvatarComponent,
  FasoIconComponent,
  FasoOtpInputComponent,
  FasoRoleChipComponent,
} from '../components-v2';
import type {
  AdminLang,
  AdminLevel,
  AdminUser,
  Capability,
  CapabilityDomain,
} from '../models/admin.model';

export interface GrantRoleDialogData {
  readonly target: AdminUser;
  readonly actorRole: AdminLevel;
  readonly lang: AdminLang;
  /**
   * Si `true`, le dialog s'ouvre directement sur l'étape Capabilities et masque
   * la sélection de rôle (utilisé depuis user-detail "Modifier les capacités").
   */
  readonly editCapsOnly?: boolean;
  /** Set initial de capacités effectives (mode edit). */
  readonly initialCapabilities?: readonly string[];
}

type Step = 1 | 2 | 3 | 4 | 5;

interface StepDef {
  readonly n: Step;
  readonly labelFr: string;
  readonly labelEn: string;
}

const STEPS: readonly StepDef[] = [
  { n: 1, labelFr: 'Sélection', labelEn: 'Selection' },
  { n: 2, labelFr: 'Capacités', labelEn: 'Capabilities' },
  { n: 3, labelFr: 'Justification', labelEn: 'Justification' },
  { n: 4, labelFr: 'OTP', labelEn: 'OTP' },
  { n: 5, labelFr: 'Résumé', labelEn: 'Summary' },
];

/**
 * Catalogue minimal de capacités (sera remplacé par GET /api/admin/capabilities).
 * Aligné sur DELTA-REQUIREMENTS-2026-04-30 §1.
 */
const CAPABILITY_REGISTRY: readonly Capability[] = [
  // Users
  { key: 'users:invite', domain: 'users', availableForRoles: ['SUPER-ADMIN', 'ADMIN'], i18nLabelKey: 'users.invite' },
  { key: 'users:suspend', domain: 'users', availableForRoles: ['SUPER-ADMIN', 'ADMIN'], i18nLabelKey: 'users.suspend' },
  { key: 'users:reactivate', domain: 'users', availableForRoles: ['SUPER-ADMIN', 'ADMIN'], i18nLabelKey: 'users.reactivate' },
  { key: 'users:view_all', domain: 'users', availableForRoles: ['SUPER-ADMIN', 'ADMIN', 'MANAGER'], i18nLabelKey: 'users.view_all' },
  // Sessions
  { key: 'sessions:list', domain: 'sessions', availableForRoles: ['SUPER-ADMIN', 'ADMIN', 'MANAGER'], i18nLabelKey: 'sessions.list' },
  { key: 'sessions:revoke', domain: 'sessions', availableForRoles: ['SUPER-ADMIN', 'ADMIN'], i18nLabelKey: 'sessions.revoke' },
  // Devices
  { key: 'devices:list', domain: 'devices', availableForRoles: ['SUPER-ADMIN', 'ADMIN', 'MANAGER'], i18nLabelKey: 'devices.list' },
  { key: 'devices:revoke', domain: 'devices', availableForRoles: ['SUPER-ADMIN', 'ADMIN'], i18nLabelKey: 'devices.revoke' },
  // MFA
  { key: 'mfa:reset', domain: 'mfa', availableForRoles: ['SUPER-ADMIN', 'ADMIN'], i18nLabelKey: 'mfa.reset' },
  { key: 'mfa:view', domain: 'mfa', availableForRoles: ['SUPER-ADMIN', 'ADMIN', 'MANAGER'], i18nLabelKey: 'mfa.view' },
  // Audit
  { key: 'audit:view', domain: 'audit', availableForRoles: ['SUPER-ADMIN', 'ADMIN', 'MANAGER'], i18nLabelKey: 'audit.view' },
  { key: 'audit:export', domain: 'audit', availableForRoles: ['SUPER-ADMIN', 'ADMIN'], i18nLabelKey: 'audit.export' },
  // Settings
  { key: 'settings:read', domain: 'settings', availableForRoles: ['SUPER-ADMIN', 'ADMIN', 'MANAGER'], i18nLabelKey: 'settings.read' },
  { key: 'settings:write_otp', domain: 'settings', availableForRoles: ['SUPER-ADMIN'], i18nLabelKey: 'settings.write_otp' },
  { key: 'settings:write_session', domain: 'settings', availableForRoles: ['SUPER-ADMIN'], i18nLabelKey: 'settings.write_session' },
  { key: 'settings:write_mfa', domain: 'settings', availableForRoles: ['SUPER-ADMIN'], i18nLabelKey: 'settings.write_mfa' },
  // Break-glass
  { key: 'break_glass:activate', domain: 'break_glass', availableForRoles: ['SUPER-ADMIN', 'ADMIN'], i18nLabelKey: 'break_glass.activate' },
  // Recovery
  { key: 'recovery:initiate', domain: 'recovery', availableForRoles: ['SUPER-ADMIN'], i18nLabelKey: 'recovery.initiate' },
  { key: 'recovery:complete', domain: 'recovery', availableForRoles: ['SUPER-ADMIN'], i18nLabelKey: 'recovery.complete' },
  // Roles
  { key: 'roles:grant_admin', domain: 'roles', availableForRoles: ['SUPER-ADMIN'], i18nLabelKey: 'roles.grant_admin' },
  { key: 'roles:grant_manager', domain: 'roles', availableForRoles: ['SUPER-ADMIN', 'ADMIN'], i18nLabelKey: 'roles.grant_manager' },
  { key: 'roles:revoke', domain: 'roles', availableForRoles: ['SUPER-ADMIN'], i18nLabelKey: 'roles.revoke' },
];

const DOMAIN_ORDER: readonly CapabilityDomain[] = [
  'users',
  'sessions',
  'devices',
  'mfa',
  'audit',
  'settings',
  'break_glass',
  'recovery',
  'roles',
];

const DOMAIN_LABEL: Record<CapabilityDomain, { fr: string; en: string }> = {
  users: { fr: 'Utilisateurs', en: 'Users' },
  sessions: { fr: 'Sessions', en: 'Sessions' },
  devices: { fr: 'Appareils', en: 'Devices' },
  mfa: { fr: 'MFA', en: 'MFA' },
  audit: { fr: 'Audit', en: 'Audit' },
  settings: { fr: 'Paramètres', en: 'Settings' },
  break_glass: { fr: 'Break-Glass', en: 'Break-Glass' },
  recovery: { fr: 'Récupération', en: 'Recovery' },
  roles: { fr: 'Rôles', en: 'Roles' },
};

@Component({
  selector: 'faso-grant-role-stepper-dialog',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [
    CommonModule,
    FormsModule,
    MatDialogModule,
    TranslateModule,
    FasoIconComponent,
    FasoAvatarComponent,
    FasoRoleChipComponent,
    FasoOtpInputComponent,
  ],
  template: `
    <div class="fd-modal" style="width: 720px;">
      <div class="fd-modal-h">
        <div class="fd-h2">
          {{
            data.lang === 'fr'
              ? 'Octroyer un rôle administrateur'
              : 'Grant admin role'
          }}
        </div>
        <div class="fd-help" style="margin-top: 4px;">
          {{ data.lang === 'fr' ? 'Acteur · ' : 'Actor · ' }}
          <faso-role-chip [role]="data.actorRole" />
          ·
          {{
            data.lang === 'fr'
              ? 'OTP audit obligatoire · trace OTel propagée'
              : 'OTP audit required · OTel trace propagated'
          }}
        </div>
      </div>

      <div style="padding: 0 22px;">
        <div class="fd-stepper">
          @for (s of steps; track s.n) {
            <div
              class="fd-step"
              [class.active]="s.n === step()"
              [class.done]="s.n < step()"
            >
              <span class="fd-step-num">
                @if (s.n < step()) {
                  <faso-icon name="check" [size]="11" />
                } @else {
                  {{ s.n }}
                }
              </span>
              <span>
                {{ data.lang === 'fr' ? s.labelFr : s.labelEn }}
              </span>
            </div>
          }
        </div>
      </div>

      <div class="fd-modal-b" style="min-height: 280px;">
        @switch (step()) {
          @case (1) {
            <div class="fd-help" style="margin-bottom: 6px;">
              {{ data.lang === 'fr' ? 'Cible' : 'Target user' }}
            </div>
            <div
              style="padding: 12px; border: 1px solid var(--border); border-radius: var(--r-sm); background: var(--surface-2); display: flex; align-items: center; gap: 12px; margin-bottom: 18px;"
            >
              <faso-avatar [user]="data.target" [size]="40" />
              <div style="flex: 1;">
                <div style="font-weight: 500;">
                  {{ data.target.firstName }} {{ data.target.lastName }}
                </div>
                <div style="font-size: 12px; color: var(--text-3);">
                  <span class="fd-mono">{{ data.target.email }}</span> ·
                  {{ data.target.department }}
                </div>
              </div>
              <faso-role-chip [role]="data.target.role" />
            </div>

            <div class="fd-help" style="margin-bottom: 8px;">
              {{
                data.lang === 'fr'
                  ? 'Rôle cible · filtré par votre niveau'
                  : 'Target role · filtered by your level'
              }}
            </div>
            <div
              style="display: grid; grid-template-columns: repeat(3, 1fr); gap: 8px;"
            >
              @for (r of allRoles; track r) {
                <div
                  [style.padding]="'14px'"
                  [style.border]="
                    targetRole() === r
                      ? '2px solid var(--primary)'
                      : '2px solid var(--border)'
                  "
                  [style.borderRadius]="'var(--r-md)'"
                  [style.cursor]="
                    isAllowed(r) ? 'pointer' : 'not-allowed'
                  "
                  [style.opacity]="isAllowed(r) ? 1 : 0.5"
                  [style.background]="
                    targetRole() === r
                      ? 'var(--primary-soft)'
                      : 'var(--surface)'
                  "
                  (click)="isAllowed(r) && targetRole.set(r)"
                >
                  <div
                    class="fd-row"
                    style="justify-content: space-between;"
                  >
                    <faso-role-chip [role]="r" />
                    @if (targetRole() === r) {
                      <faso-icon
                        name="check"
                        [size]="14"
                        style="color: var(--primary);"
                      />
                    }
                  </div>
                  <div
                    style="font-size: 11.5px; color: var(--text-3); margin-top: 8px;"
                  >
                    {{ roleDescription(r) }}
                  </div>
                  @if (!isAllowed(r)) {
                    <div
                      class="fd-help"
                      style="margin-top: 8px; color: var(--danger);"
                    >
                      {{
                        data.lang === 'fr'
                          ? 'Niveau insuffisant'
                          : 'Insufficient level'
                      }}
                    </div>
                  }
                </div>
              }
            </div>

            @if (requiresDualControl()) {
              <div
                class="fd-banner warn"
                style="margin-top: 14px; margin-bottom: 0;"
              >
                <faso-icon name="info" [size]="14" />
                <div class="fd-banner-body" style="font-size: 12px;">
                  {{
                    data.lang === 'fr'
                      ? 'Ce rôle nécessite une approbation dual-control par un autre SUPER-ADMIN.'
                      : 'This role requires dual-control approval from another SUPER-ADMIN.'
                  }}
                </div>
              </div>
            }
          }

          @case (2) {
            <div class="fd-help" style="margin-bottom: 8px;">
              {{
                data.lang === 'fr'
                  ? 'Sélectionnez les capacités octroyées à ce compte. Deux comptes même rôle ne partagent jamais le même set.'
                  : 'Select the capabilities granted to this account. No two same-role accounts share the exact same set.'
              }}
            </div>
            <div
              style="display: flex; flex-direction: column; gap: 12px; max-height: 320px; overflow: auto; padding: 4px;"
            >
              @for (group of capabilityGroups(); track group.domain) {
                <div
                  style="border: 1px solid var(--border); border-radius: var(--r-sm); padding: 10px 12px; background: var(--surface);"
                >
                  <div
                    style="font-size: 11px; text-transform: uppercase; letter-spacing: 0.06em; font-weight: 600; color: var(--text-3); margin-bottom: 6px;"
                  >
                    {{
                      data.lang === 'fr'
                        ? domainLabelFr(group.domain)
                        : domainLabelEn(group.domain)
                    }}
                  </div>
                  <div
                    style="display: flex; flex-direction: column; gap: 4px;"
                  >
                    @for (cap of group.capabilities; track cap.key) {
                      <label
                        class="fd-row"
                        style="gap: 8px; font-size: 12.5px; cursor: pointer;"
                      >
                        <input
                          type="checkbox"
                          [checked]="selectedCaps().has(cap.key)"
                          (change)="toggleCap(cap.key)"
                        />
                        <span class="fd-mono" style="font-size: 11.5px;">
                          {{ cap.key }}
                        </span>
                      </label>
                    }
                  </div>
                </div>
              }
            </div>

            <div
              style="margin-top: 12px; display: flex; gap: 8px; align-items: center;"
            >
              <button
                type="button"
                class="fd-btn ghost sm"
                (click)="checkUniqueness()"
                [disabled]="checkingUniqueness()"
              >
                <faso-icon name="check" [size]="12" />
                {{
                  data.lang === 'fr'
                    ? "Vérifier l'unicité"
                    : 'Check uniqueness'
                }}
              </button>
              <span
                class="fd-mono"
                style="font-size: 11px; color: var(--text-3);"
              >
                {{ selectedCaps().size }}
                {{ data.lang === 'fr' ? 'sélectionnées' : 'selected' }}
              </span>
            </div>

            @if (duplicateMatchEmail(); as match) {
              <div class="fd-banner warn" style="margin-top: 12px;">
                <faso-icon name="info" [size]="14" />
                <div class="fd-banner-body" style="font-size: 12px;">
                  {{
                    data.lang === 'fr'
                      ? 'Set identique à '
                      : 'Set identical to '
                  }}
                  <span class="fd-mono">{{ match }}</span>
                  {{
                    data.lang === 'fr'
                      ? ' — ajoutez ou retirez ≥ 1 capacité, ou cochez « Forcer ».'
                      : ' — add or remove ≥ 1 capability, or tick "Force".'
                  }}
                </div>
              </div>
              <label
                class="fd-row"
                style="gap: 8px; font-size: 12.5px; margin-top: 8px; cursor: pointer;"
              >
                <input
                  type="checkbox"
                  [checked]="forceDuplicate()"
                  (change)="forceDuplicate.set(!forceDuplicate())"
                />
                <span>
                  {{
                    data.lang === 'fr'
                      ? 'Forcer (audit override)'
                      : 'Force (audit override)'
                  }}
                </span>
              </label>
            }
          }

          @case (3) {
            <div
              class="fd-help"
              style="margin-bottom: 6px; display: flex; justify-content: space-between;"
            >
              <span>
                {{
                  data.lang === 'fr'
                    ? 'Justification · ≥ 50 caractères'
                    : 'Justification · ≥ 50 chars'
                }}
              </span>
              <span
                class="fd-mono"
                [style.color]="
                  justifTooShort() ? 'var(--danger)' : 'var(--ok)'
                "
              >
                {{ justification().length }} / 50
              </span>
            </div>
            <textarea
              class="fd-textarea"
              rows="6"
              [ngModel]="justification()"
              (ngModelChange)="justification.set($event)"
            ></textarea>
            <div class="fd-help" style="margin-top: 8px;">
              {{
                data.lang === 'fr'
                  ? 'Cette justification sera enregistrée dans audit_log.metadata et publiée sur Redpanda admin.role.granted.'
                  : 'Stored in audit_log.metadata and published on Redpanda admin.role.granted.'
              }}
            </div>
          }

          @case (4) {
            <div style="text-align: center; padding: 20px 0;">
              <div style="font-size: 14px; margin-bottom: 8px;">
                {{ data.lang === 'fr' ? 'Code envoyé à' : 'Code sent to' }}
                <span class="fd-mono">aminata.ouedraogo&#64;faso.bf</span>
              </div>
              <div class="fd-help" style="margin-bottom: 24px;">
                {{
                  data.lang === 'fr'
                    ? '8 chiffres · valide pendant'
                    : '8 digits · valid for'
                }}
                <span
                  class="fd-mono"
                  [style.color]="
                    countdown() < 60 ? 'var(--danger)' : 'var(--text)'
                  "
                >
                  {{ formatTime(countdown()) }}
                </span>
              </div>
              <div style="display: flex; justify-content: center;">
                <faso-otp-input
                  [length]="8"
                  [(value)]="otp"
                  (complete)="onOtpComplete($event)"
                />
              </div>
              <div
                style="margin-top: 16px; font-size: 12px; color: var(--text-3);"
              >
                {{
                  data.lang === 'fr' ? 'Pas reçu ?' : 'Didn’t receive?'
                }}
                <a class="fd-link">
                  {{ data.lang === 'fr' ? 'Renvoyer' : 'Resend' }}
                </a>
                ·
                <span class="fd-mono">
                  2 / 3
                  {{
                    data.lang === 'fr'
                      ? 'restants 5min'
                      : 'remaining 5min'
                  }}
                </span>
              </div>
            </div>
          }

          @case (5) {
            @if (submitted()) {
              <div style="padding: 22px; text-align: center;">
                <div
                  style="width: 60px; height: 60px; border-radius: 50%; background: var(--ok-soft); color: var(--ok); display: inline-flex; align-items: center; justify-content: center; margin-bottom: 14px;"
                >
                  <faso-icon name="check" [size]="30" />
                </div>
                <div class="fd-h2">
                  {{
                    data.lang === 'fr'
                      ? 'Demande enregistrée'
                      : 'Request recorded'
                  }}
                </div>
                <div class="fd-help" style="margin-top: 6px; margin-bottom: 16px;">
                  @if (requiresDualControl()) {
                    {{
                      data.lang === 'fr'
                        ? "En attente d'approbation par un autre SUPER-ADMIN."
                        : 'Awaiting approval from another SUPER-ADMIN.'
                    }}
                  } @else {
                    {{
                      data.lang === 'fr'
                        ? 'Rôle ' + targetRole() + ' octroyé immédiatement à ' + data.target.firstName + ' ' + data.target.lastName + '.'
                        : targetRole() + ' role granted immediately to ' + data.target.firstName + ' ' + data.target.lastName + '.'
                    }}
                  }
                </div>
                <div class="fd-mono-pill">trace · 9b4d0c11 ↗ Jaeger</div>
              </div>
            } @else {
              <div class="fd-help" style="margin-bottom: 10px;">
                {{
                  data.lang === 'fr'
                    ? 'Vérifiez puis confirmez. Action irréversible (rollback via révocation explicite).'
                    : 'Verify then confirm. Irreversible (rollback via explicit revoke).'
                }}
              </div>
              <div
                style="display: grid; grid-template-columns: auto 1fr; gap: 10px 16px; padding: 14px; border: 1px solid var(--border); border-radius: var(--r-md); font-size: 13px;"
              >
                <span style="color: var(--text-3);">
                  {{ data.lang === 'fr' ? 'Cible' : 'Target' }}
                </span>
                <span>
                  <strong>
                    {{ data.target.firstName }} {{ data.target.lastName }}
                  </strong>
                  ·
                  <span class="fd-mono">{{ data.target.email }}</span>
                </span>
                <span style="color: var(--text-3);">
                  {{ data.lang === 'fr' ? 'Rôle' : 'Role' }}
                </span>
                <span>
                  <faso-role-chip [role]="targetRole()" />
                  · scope=DIRECTION · tenant={{ tenantSlug() }}
                </span>
                <span style="color: var(--text-3);">
                  {{ data.lang === 'fr' ? 'Workflow' : 'Workflow' }}
                </span>
                <span>
                  @if (requiresDualControl()) {
                    {{
                      data.lang === 'fr'
                        ? 'Dual-control · 2 SUPER-ADMIN requis'
                        : 'Dual-control · 2 SUPER-ADMIN required'
                    }}
                  } @else {
                    {{
                      data.lang === 'fr'
                        ? 'Auto-approuvé'
                        : 'Auto-approved'
                    }}
                  }
                </span>
                <span style="color: var(--text-3);">
                  {{ data.lang === 'fr' ? 'Capacités' : 'Capabilities' }}
                </span>
                <span>
                  {{ selectedCaps().size }}
                  {{
                    data.lang === 'fr' ? 'octroyées' : 'granted'
                  }}
                  @if (forceDuplicate()) {
                    ·
                    <span class="fd-chip danger" style="font-size: 10.5px;">
                      {{
                        data.lang === 'fr'
                          ? 'Forcé (doublon)'
                          : 'Forced (duplicate)'
                      }}
                    </span>
                  }
                </span>
                <span style="color: var(--text-3);">
                  {{ data.lang === 'fr' ? 'Justif.' : 'Justif.' }}
                </span>
                <span style="font-style: italic; color: var(--text-2);">
                  « {{ justification() }} »
                </span>
                <span style="color: var(--text-3);">
                  {{ data.lang === 'fr' ? 'Endpoint' : 'Endpoint' }}
                </span>
                <span class="fd-mono" style="font-size: 11.5px;">
                  POST /api/admin/users/{{ data.target.id }}/roles/grant
                </span>
              </div>
            }
          }
        }
      </div>

      <div class="fd-modal-f">
        @if (!submitted() && step() > minStep()) {
          <button
            class="fd-btn"
            (click)="back()"
          >
            ← {{ data.lang === 'fr' ? 'Retour' : 'Back' }}
          </button>
        }
        <div style="flex: 1;"></div>
        @if (step() < 5) {
          <button
            class="fd-btn primary"
            [disabled]="!canAdvance()"
            (click)="next()"
          >
            {{ data.lang === 'fr' ? 'Suivant' : 'Next' }} →
          </button>
        } @else if (!submitted()) {
          <button
            class="fd-btn primary lg"
            (click)="submit()"
          >
            {{
              data.lang === 'fr'
                ? "Confirmer l'octroi"
                : 'Confirm grant'
            }}
          </button>
        } @else {
          <button class="fd-btn" (click)="close()">
            {{ data.lang === 'fr' ? 'Fermer' : 'Close' }}
          </button>
        }
      </div>
    </div>
  `,
})
export class GrantRoleStepperDialog {
  protected readonly data = inject<GrantRoleDialogData>(MAT_DIALOG_DATA);
  private readonly ref = inject(MatDialogRef<GrantRoleStepperDialog>);
  private readonly destroyRef = inject(DestroyRef);

  protected readonly steps = STEPS;
  protected readonly allRoles: readonly AdminLevel[] = [
    'SUPER-ADMIN',
    'ADMIN',
    'MANAGER',
  ];

  /** Première étape utilisable (saute "Sélection" en mode editCapsOnly). */
  protected readonly minStep = signal<Step>(this.data.editCapsOnly ? 2 : 1);

  protected readonly step = signal<Step>(this.data.editCapsOnly ? 2 : 1);
  protected readonly targetRole = signal<AdminLevel>(
    this.data.editCapsOnly ? this.data.target.role : 'MANAGER',
  );
  protected readonly justification = signal<string>(
    'Renforcement équipe ALT-MISSION pour gestion missions ministérielles Q2.',
  );
  protected readonly otp = signal<string>('');
  protected readonly countdown = signal<number>(287);
  protected readonly submitted = signal<boolean>(false);

  /** Capacités sélectionnées. Pré-remplies via `initialCapabilities` si fourni. */
  protected readonly selectedCaps = signal<Set<string>>(
    new Set(this.data.initialCapabilities ?? []),
  );
  protected readonly forceDuplicate = signal<boolean>(false);
  protected readonly duplicateMatchEmail = signal<string | null>(null);
  protected readonly checkingUniqueness = signal<boolean>(false);

  protected readonly justifTooShort = computed(
    () => this.justification().length < 50,
  );

  protected readonly otpComplete = computed(() => this.otp().length === 8);

  protected readonly requiresDualControl = computed(
    () => this.targetRole() === 'ADMIN',
  );

  protected readonly tenantSlug = computed(() => {
    const dept = this.data.target.department;
    return dept.split(' ')[0]?.toLowerCase() ?? '';
  });

  /** Capacités groupées par domaine, filtrées par rôle cible. */
  protected readonly capabilityGroups = computed(() => {
    const role = this.targetRole();
    const groups: { domain: CapabilityDomain; capabilities: Capability[] }[] = [];
    for (const domain of DOMAIN_ORDER) {
      const caps = CAPABILITY_REGISTRY.filter(
        (c) => c.domain === domain && c.availableForRoles.includes(role),
      );
      if (caps.length > 0) {
        groups.push({ domain, capabilities: caps });
      }
    }
    return groups;
  });

  protected readonly canAdvance = computed(() => {
    const s = this.step();
    if (s === 1) return this.isAllowed(this.targetRole());
    if (s === 2) {
      // Au moins 1 capacité OU forceDuplicate si doublon détecté.
      if (this.selectedCaps().size === 0) return false;
      if (this.duplicateMatchEmail() && !this.forceDuplicate()) return false;
      return true;
    }
    if (s === 3) return !this.justifTooShort();
    if (s === 4) return this.otpComplete();
    return true;
  });

  constructor() {
    // Countdown ticker — only relevant on step OTP (now step 4).
    interval(1000)
      .pipe(takeUntilDestroyed(this.destroyRef))
      .subscribe(() => {
        if (this.step() !== 4 || this.submitted()) return;
        this.countdown.update((c) => Math.max(0, c - 1));
      });
  }

  protected domainLabelFr(d: CapabilityDomain): string {
    return DOMAIN_LABEL[d].fr;
  }

  protected domainLabelEn(d: CapabilityDomain): string {
    return DOMAIN_LABEL[d].en;
  }

  protected toggleCap(key: string): void {
    const next = new Set(this.selectedCaps());
    if (next.has(key)) next.delete(key);
    else next.add(key);
    this.selectedCaps.set(next);
    // Toute modif manuelle ré-initialise l'état de duplication.
    this.duplicateMatchEmail.set(null);
    this.forceDuplicate.set(false);
  }

  protected checkUniqueness(): void {
    this.checkingUniqueness.set(true);
    // Hook future : POST /api/admin/capabilities/check-uniqueness.
    // Stub UI : déclenche un faux duplicate si exactement 4 caps sélectionnées.
    setTimeout(() => {
      this.checkingUniqueness.set(false);
      if (this.selectedCaps().size === 4) {
        this.duplicateMatchEmail.set('jane@faso.bf');
      } else {
        this.duplicateMatchEmail.set(null);
        this.forceDuplicate.set(false);
      }
    }, 250);
  }

  protected isAllowed(role: AdminLevel): boolean {
    if (this.data.actorRole === 'SUPER-ADMIN') {
      return role === 'ADMIN' || role === 'MANAGER';
    }
    if (this.data.actorRole === 'ADMIN') {
      return role === 'MANAGER';
    }
    return false;
  }

  protected roleDescription(role: AdminLevel): string {
    const fr = this.data.lang === 'fr';
    if (role === 'SUPER-ADMIN') {
      return fr
        ? 'Toute capacité plateforme.'
        : 'All platform capabilities.';
    }
    if (role === 'ADMIN') {
      return fr
        ? 'Gestion users + sessions, lecture audit.'
        : 'User + session mgmt, audit read.';
    }
    return fr
      ? 'Lecture audit, opérations métier.'
      : 'Audit read, business ops.';
  }

  protected formatTime(s: number): string {
    return `${Math.floor(s / 60)}:${(s % 60).toString().padStart(2, '0')}`;
  }

  protected next(): void {
    if (!this.canAdvance()) return;
    this.step.update((s) => Math.min(5, s + 1) as Step);
  }

  protected back(): void {
    const min = this.minStep();
    this.step.update((s) => Math.max(min, s - 1) as Step);
  }

  protected onOtpComplete(value: string): void {
    void value;
  }

  protected submit(): void {
    this.submitted.set(true);
    // Hook future : AdminUserService.grantRole(...) — body inclut désormais
    // capabilities + forceDuplicate (cf. Delta requirements §1).
  }

  protected close(): void {
    this.ref.close({
      submitted: this.submitted(),
      targetRole: this.targetRole(),
      justification: this.justification(),
      capabilities: Array.from(this.selectedCaps()),
      forceDuplicate: this.forceDuplicate(),
    });
  }
}
