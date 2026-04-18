import { ChangeDetectionStrategy, Component, Input } from '@angular/core';
import { CommonModule } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatIconModule } from '@angular/material/icon';

@Component({
  selector: 'app-category-tile',
  standalone: true,
  imports: [CommonModule, RouterLink, MatIconModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <a class="tile" [routerLink]="routerLink" [queryParams]="queryParams">
      <span class="icon-wrap"><mat-icon>{{ icon }}</mat-icon></span>
      <span class="label">{{ label }}</span>
      @if (hint) { <span class="hint">{{ hint }}</span> }
    </a>
  `,
  styles: [`
    :host { display: block; }
    .tile {
      display: flex;
      flex-direction: column;
      align-items: center;
      text-align: center;
      padding: var(--faso-space-5) var(--faso-space-3);
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
      color: var(--faso-text);
      height: 100%;
      transition:
        transform var(--faso-duration-fast) var(--faso-ease-standard),
        box-shadow var(--faso-duration-fast) var(--faso-ease-standard),
        border-color var(--faso-duration-fast) var(--faso-ease-standard);
      text-decoration: none;
    }
    .tile:hover, .tile:focus-visible {
      transform: translateY(-4px);
      box-shadow: var(--faso-elevation-hover);
      border-color: var(--faso-primary-300);
      text-decoration: none;
    }
    .icon-wrap {
      width: 56px; height: 56px;
      border-radius: 50%;
      display: inline-flex;
      align-items: center;
      justify-content: center;
      background: var(--faso-primary-50);
      color: var(--faso-primary-700);
      margin-bottom: var(--faso-space-3);
    }
    .icon-wrap mat-icon { font-size: 28px; width: 28px; height: 28px; }
    .label {
      font-weight: var(--faso-weight-semibold);
      font-size: var(--faso-text-base);
    }
    .hint {
      margin-top: 2px;
      color: var(--faso-text-muted);
      font-size: var(--faso-text-sm);
    }

    @media (prefers-reduced-motion: reduce) {
      .tile:hover, .tile:focus-visible { transform: none; }
    }
  `],
})
export class CategoryTileComponent {
  @Input({ required: true }) icon!: string;
  @Input({ required: true }) label!: string;
  @Input() hint?: string;
  @Input() routerLink: string | any[] = '.';
  @Input() queryParams?: Record<string, any>;
}
