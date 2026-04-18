// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, Input } from '@angular/core';
import { CommonModule } from '@angular/common';
import { MatProgressSpinnerModule } from '@angular/material/progress-spinner';

@Component({
  selector: 'app-loading',
  standalone: true,
  imports: [CommonModule, MatProgressSpinnerModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="loading" [class.inline]="inline">
      <mat-spinner [diameter]="diameter"></mat-spinner>
      @if (message) { <p>{{ message }}</p> }
    </div>
  `,
  styles: [`
    :host { display: block; }
    .loading {
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      gap: var(--faso-space-3);
      padding: var(--faso-space-10) var(--faso-space-4);
      color: var(--faso-text-muted);
    }
    .loading.inline { padding: var(--faso-space-3); }
    .loading p { margin: 0; font-size: var(--faso-text-sm); }
  `],
})
export class LoadingComponent {
  @Input() message = '';
  @Input() diameter = 40;
  @Input() inline = false;
}
