import { Component, Input, ChangeDetectionStrategy } from '@angular/core';
import { CommonModule } from '@angular/common';
import { MatCardModule } from '@angular/material/card';

@Component({
  selector: 'app-card',
  standalone: true,
  imports: [CommonModule, MatCardModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <mat-card [class]="'app-card ' + styleClass" [class.clickable]="clickable">
      @if (title || subtitle) {
        <mat-card-header>
          @if (icon) {
            <div mat-card-avatar class="card-icon-avatar">
              <span class="material-icons">{{ icon }}</span>
            </div>
          }
          @if (title) {
            <mat-card-title>{{ title }}</mat-card-title>
          }
          @if (subtitle) {
            <mat-card-subtitle>{{ subtitle }}</mat-card-subtitle>
          }
          <div class="card-header-actions">
            <ng-content select="[card-actions-top]"></ng-content>
          </div>
        </mat-card-header>
      }
      <mat-card-content>
        <ng-content></ng-content>
      </mat-card-content>
      @if (showFooter) {
        <mat-card-actions [align]="footerAlign">
          <ng-content select="[card-actions]"></ng-content>
        </mat-card-actions>
      }
    </mat-card>
  `,
  styles: [`
    .app-card {
      margin-bottom: 16px;
    }

    .app-card.clickable {
      cursor: pointer;
      transition: box-shadow 0.2s ease;
    }

    .app-card.clickable:hover {
      box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
    }

    .card-icon-avatar {
      display: flex;
      align-items: center;
      justify-content: center;
      background-color: var(--faso-primary, #2e7d32);
      color: white;
      border-radius: 50%;
      width: 40px;
      height: 40px;
    }

    .card-header-actions {
      margin-left: auto;
    }

    mat-card-header {
      display: flex;
      align-items: center;
    }
  `],
})
export class CardComponent {
  @Input() title = '';
  @Input() subtitle = '';
  @Input() icon = '';
  @Input() clickable = false;
  @Input() showFooter = false;
  @Input() footerAlign: 'start' | 'end' = 'end';
  @Input() styleClass = '';
}
