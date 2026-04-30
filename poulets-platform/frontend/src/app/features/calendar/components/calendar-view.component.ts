import { Component, OnInit, inject, signal, computed, ChangeDetectionStrategy } from '@angular/core';
import { CommonModule } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatCardModule } from '@angular/material/card';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatSelectModule } from '@angular/material/select';
import { MatFormFieldModule } from '@angular/material/form-field';
import { MatCheckboxModule } from '@angular/material/checkbox';
import { MatChipsModule } from '@angular/material/chips';
import { MatProgressSpinnerModule } from '@angular/material/progress-spinner';
import { MatTooltipModule } from '@angular/material/tooltip';
import { MatDialogModule, MatDialog } from '@angular/material/dialog';
import { MatDividerModule } from '@angular/material/divider';
import { TranslateModule } from '@ngx-translate/core';

import { CalendarService } from '../services/calendar.service';
import {
  CalendarEvent,
  CalendarEventType,
  CalendarFilter,
  EVENT_COLORS,
} from '../../../shared/models/calendar.models';
import { CHICKEN_RACES } from '../../../shared/models/marketplace.models';

interface CalendarDay {
  date: Date;
  dayNumber: number;
  isCurrentMonth: boolean;
  isToday: boolean;
  events: CalendarEvent[];
}

@Component({
  selector: 'app-calendar-view',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [
    CommonModule,
    RouterLink,
    MatCardModule,
    MatButtonModule,
    MatIconModule,
    MatSelectModule,
    MatFormFieldModule,
    MatCheckboxModule,
    MatChipsModule,
    MatProgressSpinnerModule,
    MatTooltipModule,
    MatDialogModule,
    MatDividerModule,
    TranslateModule,
  ],
  template: `
    <div class="calendar-page" data-testid="calendar-page">
      <div class="page-header">
        <h1>
          <mat-icon>calendar_month</mat-icon>
          {{ 'calendar.title' | translate }}
        </h1>
        <a mat-raised-button color="accent" routerLink="/calendar/planning"
           data-testid="calendar-action-create-event">
          <mat-icon>timeline</mat-icon>
          {{ 'calendar.viewPlanning' | translate }}
        </a>
      </div>

      <div class="calendar-layout">
        <!-- Sidebar: Filters -->
        <aside class="calendar-sidebar">
          <mat-card>
            <mat-card-header>
              <mat-card-title>{{ 'calendar.filters' | translate }}</mat-card-title>
            </mat-card-header>
            <mat-card-content>
              <div class="filter-section">
                <h4>{{ 'calendar.eventTypes' | translate }}</h4>
                @for (type of allEventTypes; track type.key) {
                  <div class="event-type-filter">
                    <mat-checkbox
                      [checked]="isTypeEnabled(type.key)"
                      (change)="toggleEventType(type.key)">
                      <div class="type-label">
                        <span class="color-dot" [style.background]="type.color"></span>
                        {{ 'calendar.eventType.' + type.key | translate }}
                      </div>
                    </mat-checkbox>
                  </div>
                }
              </div>

              <mat-divider></mat-divider>

              <div class="filter-section">
                <mat-checkbox
                  [checked]="myEventsOnly()"
                  (change)="myEventsOnly.set(!myEventsOnly())">
                  {{ 'calendar.myEventsOnly' | translate }}
                </mat-checkbox>
              </div>

              <!-- Legend -->
              <mat-divider></mat-divider>
              <div class="legend-section">
                <h4>{{ 'calendar.legend' | translate }}</h4>
                @for (type of allEventTypes; track type.key) {
                  <div class="legend-item">
                    <span class="color-dot" [style.background]="type.color"></span>
                    <span>{{ 'calendar.eventType.' + type.key | translate }}</span>
                  </div>
                }
              </div>
            </mat-card-content>
          </mat-card>
        </aside>

        <!-- Main: Calendar Grid -->
        <div class="calendar-main">
          <!-- Navigation -->
          <div class="calendar-nav">
            <button mat-icon-button (click)="previousMonth()" data-testid="calendar-action-prev-month">
              <mat-icon>chevron_left</mat-icon>
            </button>
            <h2 data-testid="calendar-month-label">{{ monthLabel() }}</h2>
            <button mat-icon-button (click)="nextMonth()" data-testid="calendar-action-next-month">
              <mat-icon>chevron_right</mat-icon>
            </button>
            <button mat-stroked-button (click)="goToToday()" class="today-btn"
                    data-testid="calendar-action-today">
              {{ 'calendar.today' | translate }}
            </button>
          </div>

          @if (loading()) {
            <div class="loading-container">
              <mat-spinner diameter="40"></mat-spinner>
            </div>
          } @else {
            <!-- Day headers -->
            <div class="calendar-grid" data-testid="calendar-list">
              <div class="day-header">{{ 'calendar.days.lun' | translate }}</div>
              <div class="day-header">{{ 'calendar.days.mar' | translate }}</div>
              <div class="day-header">{{ 'calendar.days.mer' | translate }}</div>
              <div class="day-header">{{ 'calendar.days.jeu' | translate }}</div>
              <div class="day-header">{{ 'calendar.days.ven' | translate }}</div>
              <div class="day-header">{{ 'calendar.days.sam' | translate }}</div>
              <div class="day-header">{{ 'calendar.days.dim' | translate }}</div>

              @for (day of calendarDays(); track day.date.toISOString()) {
                <div class="calendar-cell"
                  [class.other-month]="!day.isCurrentMonth"
                  [class.today]="day.isToday"
                  (click)="onDayClick(day)"
                  [attr.data-testid]="'calendar-list-item-' + day.date.toISOString().slice(0, 10)">
                  <span class="day-number">{{ day.dayNumber }}</span>
                  <div class="cell-events">
                    @for (event of day.events.slice(0, 3); track event.id) {
                      <div class="event-dot"
                        [style.background]="event.color"
                        [matTooltip]="event.title"
                        (click)="onEventClick(event, $event)">
                        <span class="event-label">{{ event.title }}</span>
                      </div>
                    }
                    @if (day.events.length > 3) {
                      <div class="more-events">
                        +{{ day.events.length - 3 }} {{ 'calendar.more' | translate }}
                      </div>
                    }
                  </div>
                </div>
              }
            </div>
          }
        </div>
      </div>

      <!-- Event Detail Popup -->
      @if (selectedEvent(); as event) {
        <div class="event-popup-overlay" (click)="closeEventPopup()" data-testid="calendar-modal-event-detail">
          <mat-card class="event-popup" (click)="$event.stopPropagation()">
            <mat-card-header>
              <div mat-card-avatar class="event-type-avatar"
                [style.background]="event.color">
                @switch (event.type) {
                  @case ('LOT_DISPONIBLE') { <mat-icon>egg_alt</mat-icon> }
                  @case ('LIVRAISON') { <mat-icon>local_shipping</mat-icon> }
                  @case ('CONTRAT_LIVRAISON') { <mat-icon>description</mat-icon> }
                  @case ('DEADLINE_POIDS') { <mat-icon>warning</mat-icon> }
                  @case ('VETERINAIRE') { <mat-icon>medical_services</mat-icon> }
                }
              </div>
              <mat-card-title>{{ event.title }}</mat-card-title>
              <mat-card-subtitle>{{ 'calendar.eventType.' + event.type | translate }}</mat-card-subtitle>
            </mat-card-header>
            <mat-card-content>
              @if (event.description) {
                <p>{{ event.description }}</p>
              }
              <div class="popup-details">
                <div class="popup-detail">
                  <mat-icon>event</mat-icon>
                  <span>{{ event.start | date:'medium' }}</span>
                  @if (event.end) {
                    <span> - {{ event.end | date:'medium' }}</span>
                  }
                </div>
                @if (event.race) {
                  <div class="popup-detail">
                    <mat-icon>egg_alt</mat-icon>
                    <span>{{ event.race }}</span>
                  </div>
                }
                @if (event.quantity) {
                  <div class="popup-detail">
                    <mat-icon>inventory_2</mat-icon>
                    <span>{{ event.quantity }} {{ 'calendar.units' | translate }}</span>
                  </div>
                }
                @if (event.location) {
                  <div class="popup-detail">
                    <mat-icon>location_on</mat-icon>
                    <span>{{ event.location }}</span>
                  </div>
                }
              </div>
            </mat-card-content>
            <mat-card-actions align="end">
              <button mat-button (click)="closeEventPopup()">
                {{ 'common.close' | translate }}
              </button>
            </mat-card-actions>
          </mat-card>
        </div>
      }
    </div>
  `,
  styles: [`
    .calendar-page {
      padding: 24px;
      max-width: 1400px;
      margin: 0 auto;
    }

    .page-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-bottom: 24px;
    }

    .page-header h1 {
      display: flex;
      align-items: center;
      gap: 8px;
      margin: 0;
    }

    .calendar-layout {
      display: grid;
      grid-template-columns: 260px 1fr;
      gap: 24px;
    }

    @media (max-width: 960px) {
      .calendar-layout {
        grid-template-columns: 1fr;
      }
    }

    /* Sidebar */
    .filter-section {
      margin: 12px 0;
    }

    .filter-section h4 {
      margin: 0 0 8px;
      color: #666;
      font-size: 0.85rem;
      text-transform: uppercase;
    }

    .event-type-filter {
      margin: 4px 0;
    }

    .type-label {
      display: inline-flex;
      align-items: center;
      gap: 8px;
    }

    .color-dot {
      width: 12px;
      height: 12px;
      border-radius: 50%;
      display: inline-block;
    }

    .legend-section {
      margin-top: 12px;
    }

    .legend-section h4 {
      margin: 0 0 8px;
      color: #666;
      font-size: 0.85rem;
      text-transform: uppercase;
    }

    .legend-item {
      display: flex;
      align-items: center;
      gap: 8px;
      padding: 4px 0;
      font-size: 0.85rem;
    }

    /* Calendar Navigation */
    .calendar-nav {
      display: flex;
      align-items: center;
      gap: 8px;
      margin-bottom: 16px;
    }

    .calendar-nav h2 {
      margin: 0;
      min-width: 200px;
      text-align: center;
      text-transform: capitalize;
    }

    .today-btn {
      margin-left: auto;
    }

    .loading-container {
      display: flex;
      justify-content: center;
      padding: 60px;
    }

    /* Calendar Grid */
    .calendar-grid {
      display: grid;
      grid-template-columns: repeat(7, 1fr);
      border: 1px solid #e0e0e0;
      border-radius: 8px;
      overflow: hidden;
    }

    .day-header {
      padding: 8px;
      text-align: center;
      font-weight: 600;
      font-size: 0.85rem;
      color: #666;
      background: #f5f5f5;
      border-bottom: 1px solid #e0e0e0;
    }

    .calendar-cell {
      min-height: 100px;
      padding: 4px;
      border-right: 1px solid #f0f0f0;
      border-bottom: 1px solid #f0f0f0;
      cursor: pointer;
      transition: background 0.15s;
    }

    .calendar-cell:hover {
      background: #f5f5f5;
    }

    .calendar-cell.other-month {
      opacity: 0.4;
    }

    .calendar-cell.today {
      background: #e3f2fd;
    }

    .calendar-cell.today .day-number {
      background: #1976d2;
      color: white;
      border-radius: 50%;
      width: 28px;
      height: 28px;
      display: inline-flex;
      align-items: center;
      justify-content: center;
    }

    .day-number {
      font-size: 0.85rem;
      font-weight: 500;
      display: inline-block;
      padding: 2px 4px;
    }

    .cell-events {
      display: flex;
      flex-direction: column;
      gap: 2px;
      margin-top: 2px;
    }

    .event-dot {
      padding: 2px 6px;
      border-radius: 3px;
      font-size: 0.7rem;
      color: white;
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
      cursor: pointer;
    }

    .event-label {
      overflow: hidden;
      text-overflow: ellipsis;
    }

    .more-events {
      font-size: 0.7rem;
      color: #666;
      padding: 2px 4px;
      cursor: pointer;
    }

    /* Event Popup */
    .event-popup-overlay {
      position: fixed;
      top: 0;
      left: 0;
      right: 0;
      bottom: 0;
      background: rgba(0, 0, 0, 0.3);
      display: flex;
      align-items: center;
      justify-content: center;
      z-index: 1000;
    }

    .event-popup {
      max-width: 480px;
      width: 90%;
    }

    .event-type-avatar {
      display: flex;
      align-items: center;
      justify-content: center;
      border-radius: 50%;
      color: white;
    }

    .popup-details {
      margin-top: 16px;
      display: flex;
      flex-direction: column;
      gap: 8px;
    }

    .popup-detail {
      display: flex;
      align-items: center;
      gap: 8px;
      font-size: 0.9rem;
    }

    .popup-detail mat-icon {
      font-size: 20px;
      width: 20px;
      height: 20px;
      color: #666;
    }
  `],
})
export class CalendarViewComponent implements OnInit {
  private readonly calendarService = inject(CalendarService);

  readonly allEventTypes: { key: CalendarEventType; color: string }[] = [
    { key: 'LOT_DISPONIBLE', color: EVENT_COLORS.LOT_DISPONIBLE },
    { key: 'LIVRAISON', color: EVENT_COLORS.LIVRAISON },
    { key: 'CONTRAT_LIVRAISON', color: EVENT_COLORS.CONTRAT_LIVRAISON },
    { key: 'DEADLINE_POIDS', color: EVENT_COLORS.DEADLINE_POIDS },
    { key: 'VETERINAIRE', color: EVENT_COLORS.VETERINAIRE },
  ];

  readonly currentDate = signal(new Date());
  readonly events = signal<CalendarEvent[]>([]);
  readonly loading = signal(true);
  readonly myEventsOnly = signal(false);
  readonly enabledTypes = signal<Set<CalendarEventType>>(
    new Set(['LOT_DISPONIBLE', 'LIVRAISON', 'CONTRAT_LIVRAISON', 'DEADLINE_POIDS', 'VETERINAIRE']),
  );
  readonly selectedEvent = signal<CalendarEvent | null>(null);

  readonly monthLabel = computed(() => {
    const d = this.currentDate();
    return d.toLocaleDateString('fr-FR', { month: 'long', year: 'numeric' });
  });

  readonly calendarDays = computed(() => {
    const current = this.currentDate();
    const year = current.getFullYear();
    const month = current.getMonth();
    const firstDay = new Date(year, month, 1);
    const lastDay = new Date(year, month + 1, 0);
    const today = new Date();

    // Start on Monday
    let startOffset = firstDay.getDay() - 1;
    if (startOffset < 0) startOffset = 6;

    const days: CalendarDay[] = [];
    const startDate = new Date(year, month, 1 - startOffset);

    const filteredEvents = this.events().filter(e =>
      this.enabledTypes().has(e.type),
    );

    for (let i = 0; i < 42; i++) {
      const date = new Date(startDate);
      date.setDate(startDate.getDate() + i);

      const dayEvents = filteredEvents.filter(e => {
        const eventDate = new Date(e.start);
        return eventDate.getFullYear() === date.getFullYear()
          && eventDate.getMonth() === date.getMonth()
          && eventDate.getDate() === date.getDate();
      });

      days.push({
        date: new Date(date),
        dayNumber: date.getDate(),
        isCurrentMonth: date.getMonth() === month,
        isToday: date.toDateString() === today.toDateString(),
        events: dayEvents,
      });
    }

    return days;
  });

  ngOnInit(): void {
    this.loadEvents();
  }

  isTypeEnabled(type: CalendarEventType): boolean {
    return this.enabledTypes().has(type);
  }

  toggleEventType(type: CalendarEventType): void {
    this.enabledTypes.update(types => {
      const newTypes = new Set(types);
      if (newTypes.has(type)) {
        newTypes.delete(type);
      } else {
        newTypes.add(type);
      }
      return newTypes;
    });
  }

  previousMonth(): void {
    this.currentDate.update(d => {
      const next = new Date(d);
      next.setMonth(next.getMonth() - 1);
      return next;
    });
    this.loadEvents();
  }

  nextMonth(): void {
    this.currentDate.update(d => {
      const next = new Date(d);
      next.setMonth(next.getMonth() + 1);
      return next;
    });
    this.loadEvents();
  }

  goToToday(): void {
    this.currentDate.set(new Date());
    this.loadEvents();
  }

  onDayClick(day: CalendarDay): void {
    // Could navigate to daily view or open creation dialog
  }

  onEventClick(event: CalendarEvent, mouseEvent: MouseEvent): void {
    mouseEvent.stopPropagation();
    this.selectedEvent.set(event);
  }

  closeEventPopup(): void {
    this.selectedEvent.set(null);
  }

  private loadEvents(): void {
    this.loading.set(true);
    const current = this.currentDate();
    const year = current.getFullYear();
    const month = current.getMonth();

    const dateFrom = new Date(year, month - 1, 20).toISOString();
    const dateTo = new Date(year, month + 1, 10).toISOString();

    const filter: CalendarFilter = {
      myEventsOnly: this.myEventsOnly(),
      allMarketplace: !this.myEventsOnly(),
      eventTypes: Array.from(this.enabledTypes()),
    };

    this.calendarService.getCalendarEvents(dateFrom, dateTo, filter).subscribe({
      next: (events) => {
        this.events.set(events);
        this.loading.set(false);
      },
      error: () => this.loading.set(false),
    });
  }
}
