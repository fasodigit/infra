import { ChangeDetectionStrategy, Component, Input } from '@angular/core';
import { CommonModule } from '@angular/common';
import { MatIconModule } from '@angular/material/icon';

export type TrustBadgeKind = 'halal' | 'bio' | 'vet' | 'local' | 'flag' | 'custom';

interface BadgeSpec { icon: string; label: string; cls: string; }

const SPECS: Record<Exclude<TrustBadgeKind, 'custom'>, BadgeSpec> = {
  halal:  { icon: 'verified',      label: 'Halal',         cls: 'is-success' },
  bio:    { icon: 'eco',           label: 'Bio',           cls: 'is-success' },
  vet:    { icon: 'medical_services', label: 'Vétérinaire', cls: 'is-info'   },
  local:  { icon: 'near_me',       label: 'Local',         cls: 'is-accent'  },
  flag:   { icon: 'flag',          label: 'Burkina Faso',  cls: 'is-flag'    },
};

@Component({
  selector: 'app-trust-badge',
  standalone: true,
  imports: [CommonModule, MatIconModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <span class="badge" [class]="effectiveClass" [title]="effectiveLabel">
      <mat-icon>{{ effectiveIcon }}</mat-icon>
      <span class="label">{{ effectiveLabel }}</span>
    </span>
  `,
  styles: [`
    :host { display: inline-flex; }
    .badge {
      display: inline-flex;
      align-items: center;
      gap: 4px;
      padding: 2px 8px 2px 4px;
      border-radius: var(--faso-radius-pill);
      font-size: var(--faso-text-xs);
      font-weight: var(--faso-weight-semibold);
      line-height: 1.4;
      border: 1px solid transparent;
      white-space: nowrap;
    }
    .badge mat-icon {
      font-size: 14px; width: 14px; height: 14px;
    }
    .is-success { background: var(--faso-success-bg); color: var(--faso-success); border-color: var(--faso-success); }
    .is-info    { background: var(--faso-info-bg);    color: var(--faso-info);    border-color: var(--faso-info);    }
    .is-accent  { background: var(--faso-accent-100); color: var(--faso-accent-800); border-color: var(--faso-accent-400); }
    .is-flag    { background: var(--faso-flag-yellow); color: #0F172A; border-color: var(--faso-flag-red); }
    .is-custom  { background: var(--faso-bg); color: var(--faso-text-muted); border-color: var(--faso-border); }

    :host[compact] .label, :host([compact]) .label { display: none; }
  `],
})
export class TrustBadgeComponent {
  @Input() kind: TrustBadgeKind = 'halal';
  @Input() label?: string;
  @Input() icon?: string;

  get spec(): BadgeSpec | null {
    return this.kind === 'custom' ? null : SPECS[this.kind];
  }

  get effectiveIcon(): string { return this.icon ?? this.spec?.icon ?? 'check_circle'; }
  get effectiveLabel(): string { return this.label ?? this.spec?.label ?? ''; }
  get effectiveClass(): string { return this.spec?.cls ?? 'is-custom'; }
}
