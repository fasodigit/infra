import { Component, OnInit, signal } from '@angular/core';
import { CommonModule, DatePipe } from '@angular/common';
import { ActivatedRoute, RouterLink } from '@angular/router';
import { MatCardModule } from '@angular/material/card';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { TranslateModule } from '@ngx-translate/core';
import { StatusBadgeComponent } from '../../shared/components/status-badge/status-badge.component';

interface TrackingEvent {
  date: string;
  label: string;
  description: string;
  icon: string;
}

@Component({
  selector: 'app-order-tracking',
  standalone: true,
  imports: [
    CommonModule,
    RouterLink,
    MatCardModule,
    MatButtonModule,
    MatIconModule,
    MatProgressBarModule,
    TranslateModule,
    StatusBadgeComponent,
    DatePipe,
  ],
  template: `
    <div class="tracking-container">
      <div class="page-header">
        <button mat-icon-button routerLink="../..">
          <mat-icon>arrow_back</mat-icon>
        </button>
        <h1>{{ 'orders.tracking.title' | translate }}</h1>
      </div>

      <mat-card class="progress-card">
        <mat-card-content>
          <div class="progress-info">
            <app-status-badge [status]="currentStatus()"></app-status-badge>
            <span class="progress-label">{{ progressPercent() }}%</span>
          </div>
          <mat-progress-bar mode="determinate" [value]="progressPercent()"></mat-progress-bar>
        </mat-card-content>
      </mat-card>

      <mat-card>
        <mat-card-header>
          <mat-card-title>{{ 'orders.tracking.history' | translate }}</mat-card-title>
        </mat-card-header>
        <mat-card-content>
          <div class="tracking-timeline">
            @for (event of events(); track event.date; let first = $first) {
              <div class="tracking-event" [class.latest]="first">
                <div class="event-dot">
                  <mat-icon>{{ event.icon }}</mat-icon>
                </div>
                <div class="event-content">
                  <span class="event-label">{{ event.label | translate }}</span>
                  <span class="event-desc">{{ event.description }}</span>
                  <span class="event-date">{{ event.date | date:'dd/MM/yyyy HH:mm' }}</span>
                </div>
              </div>
            }
          </div>
        </mat-card-content>
      </mat-card>
    </div>
  `,
  styles: [`
    .tracking-container {
      padding: 24px;
      max-width: 700px;
      margin: 0 auto;
    }

    .page-header {
      display: flex;
      align-items: center;
      gap: 12px;
      margin-bottom: 24px;

      h1 { margin: 0; color: var(--faso-primary-dark, #1b5e20); }
    }

    .progress-card {
      margin-bottom: 24px;

      .progress-info {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 12px;
      }

      .progress-label {
        font-size: 1.2rem;
        font-weight: 700;
        color: var(--faso-primary, #2e7d32);
      }
    }

    .tracking-timeline {
      padding: 16px 0;
    }

    .tracking-event {
      display: flex;
      gap: 16px;
      padding: 16px 0;
      position: relative;

      &:not(:last-child)::after {
        content: '';
        position: absolute;
        left: 17px;
        top: 52px;
        bottom: 0;
        width: 2px;
        background: #e0e0e0;
      }

      &.latest .event-dot {
        background: var(--faso-primary, #2e7d32);
        color: white;
      }
    }

    .event-dot {
      width: 36px;
      height: 36px;
      min-width: 36px;
      border-radius: 50%;
      background: #e0e0e0;
      display: flex;
      align-items: center;
      justify-content: center;
      color: #666;

      mat-icon { font-size: 18px; width: 18px; height: 18px; }
    }

    .event-content {
      display: flex;
      flex-direction: column;
      gap: 2px;

      .event-label { font-weight: 500; }
      .event-desc { font-size: 0.875rem; color: #666; }
      .event-date { font-size: 0.75rem; color: #999; }
    }
  `],
})
export class OrderTrackingComponent implements OnInit {
  readonly currentStatus = signal('EN_PREPARATION');
  readonly progressPercent = signal(50);
  readonly events = signal<TrackingEvent[]>([]);

  constructor(private readonly route: ActivatedRoute) {}

  ngOnInit(): void {
    this.loadTrackingEvents();
  }

  private loadTrackingEvents(): void {
    this.events.set([
      {
        date: '2026-04-06T07:00:00',
        label: 'orders.tracking.preparing',
        description: 'Les poulets sont en cours de preparation',
        icon: 'inventory',
      },
      {
        date: '2026-04-05T14:00:00',
        label: 'orders.tracking.confirmed',
        description: 'Commande confirmee par l\'eleveur',
        icon: 'check_circle',
      },
      {
        date: '2026-04-05T08:30:00',
        label: 'orders.tracking.created',
        description: 'Commande passee par le client',
        icon: 'shopping_cart',
      },
    ]);
  }
}
