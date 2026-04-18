// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

export type WorkflowStatus = 'running' | 'completed' | 'failed' | 'canceled' | 'timed_out';

export type WorkflowType =
  | 'OrderWorkflow'
  | 'HalalCertificationWorkflow'
  | 'MfaOnboardingWorkflow'
  | 'LotGrowthWorkflow'
  | 'DisputeSaga'
  | 'AnnouncePublishWorkflow';

export interface WorkflowExecution {
  id: string;
  type: WorkflowType;
  status: WorkflowStatus;
  startedAt: string;
  closedAt?: string;
  durationMs?: number;
  actorId?: string;
  actorName?: string;
  input?: Record<string, unknown>;
  result?: Record<string, unknown>;
  error?: string;
  retries: number;
  taskQueue: string;
}

export type ActivityStatus = 'pending' | 'running' | 'completed' | 'failed' | 'retried';

export interface ActivityRun {
  id: string;
  name: string;
  status: ActivityStatus;
  startedAt: string;
  closedAt?: string;
  durationMs?: number;
  attempt: number;
  error?: string;
}

export interface WorkflowHistoryEvent {
  id: string;
  timestamp: string;
  eventType:
    | 'WorkflowExecutionStarted'
    | 'ActivityTaskScheduled'
    | 'ActivityTaskStarted'
    | 'ActivityTaskCompleted'
    | 'ActivityTaskFailed'
    | 'WorkflowSignaled'
    | 'TimerStarted'
    | 'TimerFired'
    | 'WorkflowExecutionCompleted'
    | 'WorkflowExecutionFailed'
    | 'WorkflowExecutionCanceled';
  payload?: Record<string, unknown>;
}

export interface WorkflowLatency {
  type: WorkflowType;
  p50Ms: number;
  p95Ms: number;
  p99Ms: number;
  p99_7dMs: number;
  count24h: number;
  failRate24h: number;
}
