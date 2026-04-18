import { ChangeDetectionStrategy, Component, Input } from '@angular/core';
import { CommonModule } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatIconModule } from '@angular/material/icon';

@Component({
  selector: 'app-section-header',
  standalone: true,
  imports: [CommonModule, RouterLink, MatIconModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <header class="section">
      <div>
        @if (kicker) { <span class="kicker">{{ kicker }}</span> }
        <h2 class="title">{{ title }}</h2>
        @if (subtitle) { <p class="subtitle">{{ subtitle }}</p> }
      </div>
      @if (linkLabel && linkTo) {
        <a class="more" [routerLink]="linkTo">
          {{ linkLabel }}
          <mat-icon>arrow_forward</mat-icon>
        </a>
      }
    </header>
  `,
  styles: [`
    .section {
      display: flex;
      align-items: flex-end;
      justify-content: space-between;
      gap: var(--faso-space-4);
      margin-bottom: var(--faso-space-6);
    }
    .kicker {
      display: inline-block;
      color: var(--faso-accent-700);
      font-weight: var(--faso-weight-semibold);
      font-size: var(--faso-text-sm);
      text-transform: uppercase;
      letter-spacing: 0.08em;
      margin-bottom: var(--faso-space-1);
    }
    .title {
      font-size: var(--faso-text-3xl);
      font-weight: var(--faso-weight-bold);
      line-height: var(--faso-leading-tight);
      color: var(--faso-text);
      margin: 0;
    }
    .subtitle {
      margin: var(--faso-space-2) 0 0;
      color: var(--faso-text-muted);
      max-width: 60ch;
    }
    .more {
      display: inline-flex;
      align-items: center;
      gap: 4px;
      color: var(--faso-primary-600);
      font-weight: var(--faso-weight-semibold);
      white-space: nowrap;
    }
    .more mat-icon { font-size: 18px; width: 18px; height: 18px; }

    @media (max-width: 639px) {
      .section { flex-direction: column; align-items: flex-start; }
      .title { font-size: var(--faso-text-2xl); }
    }
  `],
})
export class SectionHeaderComponent {
  @Input({ required: true }) title!: string;
  @Input() kicker?: string;
  @Input() subtitle?: string;
  @Input() linkLabel?: string;
  @Input() linkTo?: string | any[];
}
