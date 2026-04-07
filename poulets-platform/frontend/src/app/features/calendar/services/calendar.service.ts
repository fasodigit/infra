import { Injectable, inject } from '@angular/core';
import { Apollo } from 'apollo-angular';
import { Observable, map, filter as rxFilter } from 'rxjs';
import {
  CalendarEvent,
  CalendarFilter,
  PlanningData,
  PlanningFilter,
} from '../../../shared/models/calendar.models';
import {
  GET_CALENDAR_EVENTS,
  GET_MY_EVENTS,
  GET_PLANNING_DATA,
} from '../graphql/calendar.graphql';

@Injectable({ providedIn: 'root' })
export class CalendarService {
  private readonly apollo = inject(Apollo);

  getCalendarEvents(dateFrom: string, dateTo: string, filter?: CalendarFilter): Observable<CalendarEvent[]> {
    return this.apollo
      .watchQuery<{ calendarEvents: CalendarEvent[] }>({
        query: GET_CALENDAR_EVENTS,
        variables: { dateFrom, dateTo, filter },
        fetchPolicy: 'network-only',
      })
      .valueChanges.pipe(
        rxFilter((r) => !!r.data),
        map((r) => r.data!.calendarEvents as CalendarEvent[]),
      );
  }

  getMyEvents(dateFrom: string, dateTo: string): Observable<CalendarEvent[]> {
    return this.apollo
      .watchQuery<{ myCalendarEvents: CalendarEvent[] }>({
        query: GET_MY_EVENTS,
        variables: { dateFrom, dateTo },
        fetchPolicy: 'network-only',
      })
      .valueChanges.pipe(
        rxFilter((r) => !!r.data),
        map((r) => r.data!.myCalendarEvents as CalendarEvent[]),
      );
  }

  getPlanningData(filter: PlanningFilter): Observable<PlanningData[]> {
    return this.apollo
      .watchQuery<{ planningData: PlanningData[] }>({
        query: GET_PLANNING_DATA,
        variables: { filter },
        fetchPolicy: 'network-only',
      })
      .valueChanges.pipe(
        rxFilter((r) => !!r.data),
        map((r) => r.data!.planningData as PlanningData[]),
      );
  }
}
