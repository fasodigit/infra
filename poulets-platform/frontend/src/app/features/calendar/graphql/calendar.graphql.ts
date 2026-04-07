import { gql } from 'apollo-angular';

export const GET_CALENDAR_EVENTS = gql`
  query GetCalendarEvents($dateFrom: String!, $dateTo: String!, $filter: CalendarFilterInput) {
    calendarEvents(dateFrom: $dateFrom, dateTo: $dateTo, filter: $filter) {
      id
      title
      description
      start
      end
      allDay
      type
      color
      relatedId
      race
      quantity
      location
    }
  }
`;

export const GET_PLANNING_DATA = gql`
  query GetPlanningData($filter: PlanningFilterInput!) {
    planningData(filter: $filter) {
      race
      weeks {
        weekStart
        weekEnd
        weekLabel
        supply
        demand
        gap
      }
    }
  }
`;

export const GET_MY_EVENTS = gql`
  query GetMyEvents($dateFrom: String!, $dateTo: String!) {
    myCalendarEvents(dateFrom: $dateFrom, dateTo: $dateTo) {
      id
      title
      description
      start
      end
      allDay
      type
      color
      relatedId
      race
      quantity
      location
    }
  }
`;
