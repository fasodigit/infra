import { Component, Input, ChangeDetectionStrategy } from '@angular/core';
import { CommonModule } from '@angular/common';
import { StatusLabelPipe, STATUS_COLORS } from '../../pipes/status-label.pipe';

@Component({
  selector: 'app-status-badge',
  standalone: true,
  imports: [CommonModule, StatusLabelPipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <span
      class="status-badge"
      [style.background-color]="getColor()"
      [style.color]="'white'"
    >
      {{ status | statusLabel }}
    </span>
  `,
  styles: [`
    .status-badge {
      display: inline-flex;
      align-items: center;
      padding: 4px 12px;
      border-radius: 16px;
      font-size: 0.75rem;
      font-weight: 500;
      letter-spacing: 0.02em;
      white-space: nowrap;
    }
  `],
})
export class StatusBadgeComponent {
  @Input() status = '';

  getColor(): string {
    return STATUS_COLORS[this.status] || '#9e9e9e';
  }
}
