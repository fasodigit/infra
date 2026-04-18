// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { Injectable, inject } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Observable, delay, of } from 'rxjs';
import { environment } from '@env/environment';
import {
  WorkflowExecution, WorkflowStatus, WorkflowType, WorkflowHistoryEvent,
  WorkflowLatency, ActivityRun,
} from '../models';

/**
 * Wrapper pour les endpoints BFF exposant Temporal Java SDK côté backend.
 * Endpoints attendus (à implémenter côté poulets-bff) :
 *   GET    /api/admin/workflows?type=&status=&actor=
 *   GET    /api/admin/workflows/:id
 *   GET    /api/admin/workflows/:id/history
 *   POST   /api/admin/workflows/:id/signal
 *   POST   /api/admin/workflows/:id/cancel
 *   POST   /api/admin/workflows/:id/terminate
 *   GET    /api/admin/workflows/stats/latency
 *
 * Temporal UI native : http://localhost:8088 (via podman-compose temporal stack)
 */
@Injectable({ providedIn: 'root' })
export class TemporalWorkflowsService {
  private readonly http = inject(HttpClient);
  private readonly api = ((environment as any).bffUrl ?? '/api') + '/admin/workflows';
  readonly temporalUiUrl = (environment as any).temporalUiUrl ?? 'http://localhost:8088';

  list(filters?: { type?: WorkflowType; status?: WorkflowStatus; actorId?: string }): Observable<WorkflowExecution[]> {
    // Stub dev : retourne mock. En prod : this.http.get<WorkflowExecution[]>(this.api, { params }).
    return of(MOCK_WORKFLOWS.filter((w) => {
      if (filters?.type && w.type !== filters.type) return false;
      if (filters?.status && w.status !== filters.status) return false;
      if (filters?.actorId && w.actorId !== filters.actorId) return false;
      return true;
    })).pipe(delay(120));
  }

  get(id: string): Observable<WorkflowExecution | null> {
    return of(MOCK_WORKFLOWS.find((w) => w.id === id) ?? null).pipe(delay(80));
  }

  history(id: string): Observable<WorkflowHistoryEvent[]> {
    return of(MOCK_HISTORY[id] ?? []).pipe(delay(100));
  }

  activities(id: string): Observable<ActivityRun[]> {
    return of(MOCK_ACTIVITIES[id] ?? []).pipe(delay(100));
  }

  signal(id: string, name: string, payload?: unknown): Observable<boolean> {
    // Stub
    return of(true).pipe(delay(200));
  }

  cancel(id: string, reason?: string): Observable<boolean> {
    return of(true).pipe(delay(200));
  }

  terminate(id: string, reason: string): Observable<boolean> {
    return of(true).pipe(delay(200));
  }

  latencyStats(): Observable<WorkflowLatency[]> {
    return of(MOCK_LATENCIES).pipe(delay(100));
  }

  /** Deep-link vers l'UI Temporal native. */
  temporalUiLink(workflowId: string, namespace = 'default'): string {
    return `${this.temporalUiUrl}/namespaces/${namespace}/workflows/${workflowId}`;
  }
}

// --------------------------------------------------------- Mock data

const now = Date.now();

const MOCK_WORKFLOWS: WorkflowExecution[] = [
  { id: 'wf-order-A8X12',    type: 'OrderWorkflow',              status: 'running',   startedAt: new Date(now - 3 * 3600000).toISOString(),  actorName: 'Fatim Compaoré', taskQueue: 'orders',       retries: 0 },
  { id: 'wf-order-A8X11',    type: 'OrderWorkflow',              status: 'completed', startedAt: new Date(now - 48 * 3600000).toISOString(), closedAt: new Date(now - 4 * 3600000).toISOString(), durationMs: 44 * 3600000, actorName: 'Issouf Bandé', taskQueue: 'orders', retries: 0 },
  { id: 'wf-halal-L041',     type: 'HalalCertificationWorkflow', status: 'running',   startedAt: new Date(now - 5 * 86400000).toISOString(), actorName: 'Kassim Ouédraogo', taskQueue: 'halal', retries: 2 },
  { id: 'wf-mfa-u-7',        type: 'MfaOnboardingWorkflow',      status: 'running',   startedAt: new Date(now - 4 * 86400000).toISOString(), actorName: 'Mariam Sawadogo', taskQueue: 'mfa', retries: 0 },
  { id: 'wf-growth-L041',    type: 'LotGrowthWorkflow',          status: 'running',   startedAt: new Date(now - 30 * 86400000).toISOString(), actorName: 'Kassim Ouédraogo', taskQueue: 'growth', retries: 0 },
  { id: 'wf-dispute-D42',    type: 'DisputeSaga',                status: 'failed',    startedAt: new Date(now - 12 * 86400000).toISOString(), closedAt: new Date(now - 6 * 86400000).toISOString(), durationMs: 6 * 86400000, error: 'Four-eyes timeout (14d SLA)', actorName: 'Disputed client', taskQueue: 'disputes', retries: 3 },
  { id: 'wf-publish-anc-77', type: 'AnnouncePublishWorkflow',    status: 'completed', startedAt: new Date(now - 2 * 3600000).toISOString(),  closedAt: new Date(now - 2 * 3600000 + 180000).toISOString(), durationMs: 180000, actorName: 'Awa Sankara', taskQueue: 'publish', retries: 0 },
];

const MOCK_HISTORY: Record<string, WorkflowHistoryEvent[]> = {
  'wf-order-A8X12': [
    { id: '1', timestamp: new Date(now - 3 * 3600000).toISOString(),      eventType: 'WorkflowExecutionStarted', payload: { orderId: 'A8X12', amount: 45000 } },
    { id: '2', timestamp: new Date(now - 3 * 3600000 + 500).toISOString(), eventType: 'ActivityTaskScheduled',    payload: { name: 'sendConfirmationEmail' } },
    { id: '3', timestamp: new Date(now - 3 * 3600000 + 1500).toISOString(),eventType: 'ActivityTaskCompleted',    payload: { name: 'sendConfirmationEmail' } },
    { id: '4', timestamp: new Date(now - 3 * 3600000 + 2000).toISOString(),eventType: 'ActivityTaskScheduled',    payload: { name: 'reservePouletsFromStock' } },
    { id: '5', timestamp: new Date(now - 3 * 3600000 + 2500).toISOString(),eventType: 'ActivityTaskCompleted',    payload: { name: 'reservePouletsFromStock' } },
    { id: '6', timestamp: new Date(now - 3 * 3600000 + 3000).toISOString(),eventType: 'TimerStarted',             payload: { duration: '24h', purpose: 'awaitConfirmation' } },
  ],
  'wf-halal-L041': [
    { id: '1', timestamp: new Date(now - 5 * 86400000).toISOString(), eventType: 'WorkflowExecutionStarted', payload: { lotId: 'L-2026-041', stepCount: 6 } },
    { id: '2', timestamp: new Date(now - 5 * 86400000 + 3600000).toISOString(), eventType: 'WorkflowSignaled', payload: { signal: 'stepCompleted', step: 1 } },
    { id: '3', timestamp: new Date(now - 4 * 86400000).toISOString(), eventType: 'WorkflowSignaled', payload: { signal: 'stepCompleted', step: 2 } },
    { id: '4', timestamp: new Date(now - 2 * 86400000).toISOString(), eventType: 'ActivityTaskFailed', payload: { name: 'awaitAdminApproval', error: 'Timeout 72h' } },
    { id: '5', timestamp: new Date(now - 2 * 86400000 + 1000).toISOString(), eventType: 'ActivityTaskScheduled', payload: { name: 'awaitAdminApproval', attempt: 2 } },
  ],
};

const MOCK_ACTIVITIES: Record<string, ActivityRun[]> = {
  'wf-order-A8X12': [
    { id: 'a1', name: 'sendConfirmationEmail',    status: 'completed', startedAt: new Date(now - 3 * 3600000 + 500).toISOString(),  closedAt: new Date(now - 3 * 3600000 + 1500).toISOString(), durationMs: 1000, attempt: 1 },
    { id: 'a2', name: 'reservePouletsFromStock',  status: 'completed', startedAt: new Date(now - 3 * 3600000 + 2000).toISOString(), closedAt: new Date(now - 3 * 3600000 + 2500).toISOString(), durationMs: 500,  attempt: 1 },
    { id: 'a3', name: 'awaitEleveurConfirmation', status: 'running',   startedAt: new Date(now - 3 * 3600000 + 3000).toISOString(), attempt: 1 },
  ],
  'wf-halal-L041': [
    { id: 'a1', name: 'verifyElevageCompliance',  status: 'completed', startedAt: new Date(now - 5 * 86400000).toISOString(),     closedAt: new Date(now - 4.5 * 86400000).toISOString(), durationMs: 43200000, attempt: 1 },
    { id: 'a2', name: 'verifyLotIdentification',  status: 'completed', startedAt: new Date(now - 4 * 86400000).toISOString(),     closedAt: new Date(now - 3.5 * 86400000).toISOString(), durationMs: 43200000, attempt: 1 },
    { id: 'a3', name: 'awaitAdminApproval',       status: 'retried',   startedAt: new Date(now - 2 * 86400000 + 1000).toISOString(), attempt: 2, error: 'Previous attempt: timeout 72h' },
  ],
};

const MOCK_LATENCIES: WorkflowLatency[] = [
  { type: 'OrderWorkflow',              p50Ms: 2 * 86400000,  p95Ms: 5 * 86400000,  p99Ms: 7 * 86400000,  p99_7dMs: 6 * 86400000,  count24h: 72, failRate24h: 0.04 },
  { type: 'HalalCertificationWorkflow', p50Ms: 7 * 86400000,  p95Ms: 15 * 86400000, p99Ms: 28 * 86400000, p99_7dMs: 22 * 86400000, count24h: 5,  failRate24h: 0.08 },
  { type: 'MfaOnboardingWorkflow',      p50Ms: 10 * 86400000, p95Ms: 25 * 86400000, p99Ms: 30 * 86400000, p99_7dMs: 28 * 86400000, count24h: 18, failRate24h: 0.11 },
  { type: 'LotGrowthWorkflow',          p50Ms: 48 * 86400000, p95Ms: 55 * 86400000, p99Ms: 62 * 86400000, p99_7dMs: 58 * 86400000, count24h: 1,  failRate24h: 0.00 },
  { type: 'DisputeSaga',                p50Ms: 5 * 86400000,  p95Ms: 11 * 86400000, p99Ms: 14 * 86400000, p99_7dMs: 12 * 86400000, count24h: 2,  failRate24h: 0.50 },
  { type: 'AnnouncePublishWorkflow',    p50Ms: 120000,        p95Ms: 240000,        p99Ms: 300000,        p99_7dMs: 280000,        count24h: 186, failRate24h: 0.01 },
];
