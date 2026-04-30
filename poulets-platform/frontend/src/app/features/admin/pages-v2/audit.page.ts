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
import { FormsModule } from '@angular/forms';
import { TranslateModule } from '@ngx-translate/core';
import {
  FasoAvatarComponent,
  FasoIconComponent,
} from '../components-v2';
import type {
  AdminLang,
  AdminUser,
  AuditAction,
  AuditEntry,
} from '../models/admin.model';
import { MOCK_AUDIT, MOCK_USERS } from '../services/admin-mocks';

const ALL_ACTIONS: readonly AuditAction[] = [
  'USER_CREATED',
  'ROLE_GRANTED',
  'OTP_ISSUED',
  'OTP_VERIFIED',
  'OTP_FAILED',
  'MFA_ENROLLED',
  'DEVICE_TRUSTED',
  'SESSION_REVOKED',
  'BREAK_GLASS_ACTIVATED',
  'SETTINGS_UPDATED',
  // Delta 2026-04-30 — protection SUPER-ADMIN + capabilities
  'SUPER_ADMIN_PROTECTION_TRIGGERED',
  'CAPABILITY_GRANTED',
  'CAPABILITY_REVOKED',
  'CAPABILITY_SET_DUPLICATE_OVERRIDE',
  // Delta 2026-04-30 — recovery
  'ACCOUNT_RECOVERY_SELF_INITIATED',
  'ACCOUNT_RECOVERY_ADMIN_INITIATED',
  'ACCOUNT_RECOVERY_COMPLETED',
  'RECOVERY_CODE_INVALID',
  'RECOVERY_CODE_USED',
  // Phase 4.b.3 — Crypto upgrade Argon2id + HMAC pepper
  'HASH_REHASHED_ON_LOGIN',
];

@Component({
  selector: 'faso-audit-page',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [
    CommonModule,
    FormsModule,
    TranslateModule,
    FasoIconComponent,
    FasoAvatarComponent,
  ],
  template: `
    <div class="fd-page-head">
      <div>
        <div class="fd-h1">
          {{ lang() === 'fr' ? "Journal d'audit" : 'Audit log' }}
        </div>
        <div class="fd-page-sub">
          {{
            lang() === 'fr'
              ? 'Append-only · rétention 7 ans (Loi 010-2004 BF) · liens Jaeger pour chaque trace.'
              : 'Append-only · 7-year retention · Jaeger links per trace.'
          }}
        </div>
      </div>
      <div class="fd-row">
        <button class="fd-btn">
          <faso-icon name="download" [size]="13" /> CSV
        </button>
        <button class="fd-btn">
          <faso-icon name="download" [size]="13" /> JSON
        </button>
      </div>
    </div>

    <div style="display: grid; grid-template-columns: 260px 1fr; gap: 16px;">
      <aside class="fd-card" style="padding: 16px; align-self: flex-start;">
        <div style="font-weight: 600; margin-bottom: 12px; font-size: 13px;">
          {{ lang() === 'fr' ? 'Filtres' : 'Filters' }}
        </div>

        <div style="margin-bottom: 12px;">
          <div class="fd-help" style="margin-bottom: 4px;">
            {{ lang() === 'fr' ? 'Période' : 'Date range' }}
          </div>
          <div
            style="display: grid; grid-template-columns: 1fr 1fr; gap: 6px;"
          >
            <input
              class="fd-input"
              [ngModel]="dateFrom()"
              (ngModelChange)="dateFrom.set($event)"
            />
            <input
              class="fd-input"
              [ngModel]="dateTo()"
              (ngModelChange)="dateTo.set($event)"
            />
          </div>
        </div>

        <div style="margin-bottom: 12px;">
          <div class="fd-help" style="margin-bottom: 4px;">
            {{ lang() === 'fr' ? 'Acteur' : 'Actor' }}
          </div>
          <input
            class="fd-input"
            [placeholder]="
              lang() === 'fr' ? 'email ou nom…' : 'email or name…'
            "
            [ngModel]="actor()"
            (ngModelChange)="actor.set($event)"
          />
        </div>

        <div style="margin-bottom: 12px;">
          <div class="fd-help" style="margin-bottom: 6px;">
            {{ lang() === 'fr' ? "Types d'action" : 'Action types' }}
          </div>
          <div
            style="display: flex; flex-wrap: wrap; gap: 4px; max-height: 180px; overflow: auto; padding: 4px; border: 1px solid var(--border); border-radius: var(--r-sm);"
          >
            @for (a of allActions; track a) {
              <span
                class="fd-chip"
                [class.role-admin]="selectedActions().has(a)"
                [class.muted]="!selectedActions().has(a)"
                style="font-size: 10.5px; cursor: pointer;"
                (click)="toggleAction(a)"
              >
                @if (selectedActions().has(a)) {
                  <faso-icon name="check" [size]="9" />
                }
                {{ a }}
              </span>
            }
          </div>
        </div>

        <div style="margin-bottom: 12px;">
          <div class="fd-help" style="margin-bottom: 4px;">IP / CIDR</div>
          <input
            class="fd-input fd-mono"
            placeholder="196.28.111.0/24"
            [ngModel]="ipCidr()"
            (ngModelChange)="ipCidr.set($event)"
          />
        </div>

        <div style="margin-bottom: 12px;">
          <label class="fd-row" style="gap: 8px; font-size: 12.5px;">
            <input
              type="checkbox"
              [checked]="criticalOnly()"
              (change)="criticalOnly.set(!criticalOnly())"
            />
            {{
              lang() === 'fr'
                ? 'Afficher uniquement critiques'
                : 'Critical only'
            }}
          </label>
        </div>

        <button
          class="fd-btn primary"
          style="width: 100%; justify-content: center;"
          (click)="apply()"
        >
          {{ lang() === 'fr' ? 'Appliquer' : 'Apply' }}
        </button>
        <button
          class="fd-btn ghost sm"
          style="width: 100%; justify-content: center; margin-top: 6px;"
          (click)="reset()"
        >
          {{ lang() === 'fr' ? 'Réinitialiser' : 'Reset' }}
        </button>
      </aside>

      <div>
        <div
          class="fd-card"
          style="margin-bottom: 16px; padding: 12px 16px; display: flex; align-items: center; gap: 14px;"
        >
          <span class="fd-chip muted">
            {{ entries().length }}
            {{ lang() === 'fr' ? 'résultats' : 'results' }}
          </span>
          <span class="fd-chip danger">
            {{ criticalCount() }}
            {{ lang() === 'fr' ? 'critiques' : 'critical' }}
          </span>
          <span style="flex: 1;"></span>
          <span class="fd-help">
            {{
              lang() === 'fr' ? 'Trier · plus récent' : 'Sort · most recent'
            }}
          </span>
        </div>

        <div class="fd-card">
          <div style="padding: 0 18px;">
            @for (a of entries(); track a.id; let idx = $index) {
              <div
                [style.padding]="'14px 0'"
                [style.borderBottom]="
                  idx < entries().length - 1
                    ? '1px solid var(--border)'
                    : 'none'
                "
              >
                <div
                  style="display: flex; align-items: flex-start; gap: 14px;"
                >
                  <div
                    [style.marginTop]="'2px'"
                    [style.color]="
                      a.critical ? 'var(--danger)' : 'var(--primary)'
                    "
                  >
                    @if (a.critical) {
                      <faso-icon name="alertTri" [size]="16" />
                    } @else {
                      <faso-icon name="check" [size]="16" />
                    }
                  </div>

                  <div style="flex: 1; min-width: 0;">
                    <div class="fd-row" style="gap: 10px;">
                      <span
                        class="fd-chip fd-mono"
                        [class.danger]="a.critical"
                        [class.role-admin]="!a.critical"
                        style="font-size: 11px;"
                      >
                        {{ a.action }}
                      </span>
                      @if (actorOf(a); as actor) {
                        <span style="font-size: 13px; font-weight: 500;">
                          {{ actor.firstName }} {{ actor.lastName }}
                        </span>
                        <span class="fd-chip muted" style="font-size: 10.5px;">
                          {{ actor.role }}
                        </span>
                      }
                    </div>
                    <div
                      style="color: var(--text-2); font-size: 12.5px; margin-top: 6px;"
                    >
                      {{ a.desc }}
                    </div>
                    <div
                      class="fd-row"
                      style="gap: 10px; margin-top: 8px; font-size: 11.5px; color: var(--text-3);"
                    >
                      <span>
                        <faso-icon name="clock" [size]="11" />
                        {{ a.date }} ·
                        <span class="fd-mono">{{ a.time }}</span>
                      </span>
                      <span>·</span>
                      <span>
                        <faso-icon name="globe" [size]="11" />
                        <span class="fd-mono">{{ a.ip }}</span>
                      </span>
                      <span>·</span>
                      <a class="fd-link fd-mono" style="font-size: 11.5px;">
                        trace · {{ a.traceId }} ↗
                      </a>
                    </div>

                    @if (
                      expanded() === a.id &&
                      a.oldVal !== undefined
                    ) {
                      <div style="margin-top: 12px;">
                        <div class="fd-help" style="margin-bottom: 6px;">
                          {{ lang() === 'fr' ? 'Diff' : 'Diff' }}
                        </div>
                        <div class="fd-diff">
                          <div class="fd-diff-block fd-diff-old">
                            - "{{ a.target }}":
                            {{ stringify(a.oldVal) }}
                          </div>
                          <div class="fd-diff-block fd-diff-new">
                            + "{{ a.target }}":
                            {{ stringify(a.newVal) }}
                          </div>
                        </div>
                      </div>
                    }
                  </div>

                  <button
                    class="fd-btn ghost sm"
                    (click)="toggleExpand(a.id)"
                  >
                    @if (expanded() === a.id) {
                      <faso-icon name="chevD" [size]="12" />
                    } @else {
                      <faso-icon name="chevR" [size]="12" />
                    }
                  </button>
                </div>
              </div>
            }
          </div>
        </div>
      </div>
    </div>
  `,
  styles: [`:host { display: contents; }`],
})
export class AuditPage {
  readonly lang = input<AdminLang>('fr');

  protected readonly allActions = ALL_ACTIONS;

  protected readonly dateFrom = signal<string>('29/04/2026');
  protected readonly dateTo = signal<string>('30/04/2026');
  protected readonly actor = signal<string>('');
  protected readonly selectedActions = signal<Set<AuditAction>>(
    new Set<AuditAction>(['USER_CREATED', 'ROLE_GRANTED', 'OTP_ISSUED']),
  );
  protected readonly ipCidr = signal<string>('');
  protected readonly criticalOnly = signal<boolean>(true);

  protected readonly entries = signal<readonly AuditEntry[]>(MOCK_AUDIT);
  protected readonly users = signal<readonly AdminUser[]>(MOCK_USERS);

  protected readonly expanded = signal<string | null>(MOCK_AUDIT[0]?.id ?? null);

  protected readonly criticalCount = computed(
    () => this.entries().filter((a) => a.critical).length,
  );

  protected actorOf(entry: AuditEntry): AdminUser | undefined {
    return this.users().find((u) => u.id === entry.actor);
  }

  protected toggleAction(a: AuditAction): void {
    const next = new Set(this.selectedActions());
    if (next.has(a)) {
      next.delete(a);
    } else {
      next.add(a);
    }
    this.selectedActions.set(next);
  }

  protected toggleExpand(id: string): void {
    this.expanded.update((cur) => (cur === id ? null : id));
  }

  protected stringify(v: unknown): string {
    return JSON.stringify(v);
  }

  protected apply(): void {
    // Hook future: AdminAuditService.query(filters).
  }

  protected reset(): void {
    this.dateFrom.set('');
    this.dateTo.set('');
    this.actor.set('');
    this.selectedActions.set(new Set());
    this.ipCidr.set('');
    this.criticalOnly.set(false);
  }
}
