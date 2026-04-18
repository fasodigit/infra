// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { Injectable, computed, signal, inject, PLATFORM_ID } from '@angular/core';
import { isPlatformBrowser } from '@angular/common';
import { Observable, of } from 'rxjs';

import {
  ServiceHealth, SystemAlert, ActiveUsersStat,
} from '@shared/models/admin.models';

/**
 * Signal-based platform health store.
 * Adapted from shared-infrastructure/admin-ui PlatformHealthStore (originally NgRx signalStore)
 * — rewritten in native Angular signals to avoid a new dependency.
 *
 * Polling stub: in production, connect to BFF `/api/monitoring/health` or SSE endpoint.
 */
@Injectable({ providedIn: 'root' })
export class PlatformHealthStore {
  private readonly platformId = inject(PLATFORM_ID);

  // --------------------------------------------------------- state
  private readonly _services = signal<ServiceHealth[]>([]);
  private readonly _alerts = signal<SystemAlert[]>([]);
  private readonly _activeUsers = signal<ActiveUsersStat>({ total: 0, byRole: { ELEVEUR: 0, CLIENT: 0, PRODUCTEUR: 0, ADMIN: 0 } });
  private readonly _connected = signal(false);
  private readonly _lastSync = signal<string | null>(null);

  // --------------------------------------------------------- selectors
  readonly services = this._services.asReadonly();
  readonly alerts = this._alerts.asReadonly();
  readonly activeUsers = this._activeUsers.asReadonly();
  readonly connected = this._connected.asReadonly();
  readonly lastSync = this._lastSync.asReadonly();

  readonly overallStatus = computed<'HEALTHY' | 'DEGRADED' | 'CRITICAL'>(() => {
    const s = this._services();
    if (s.some((x) => x.status === 'DOWN')) return 'CRITICAL';
    if (s.some((x) => x.status === 'DEGRADED')) return 'DEGRADED';
    return 'HEALTHY';
  });

  readonly criticalAlerts = computed(() =>
    this._alerts().filter((a) => a.severity === 'critical' && !a.acknowledged),
  );

  readonly totalActiveUsers = computed(() => this._activeUsers().total);

  // --------------------------------------------------------- mutations
  updateServiceHealth(svc: ServiceHealth): void {
    const rest = this._services().filter((s) => s.name !== svc.name);
    this._services.set([...rest, svc]);
    this._lastSync.set(new Date().toISOString());
  }

  setServices(services: ServiceHealth[]): void {
    this._services.set(services);
    this._lastSync.set(new Date().toISOString());
  }

  addAlert(alert: SystemAlert): void {
    this._alerts.set([alert, ...this._alerts()].slice(0, 100));
  }

  setAlerts(alerts: SystemAlert[]): void { this._alerts.set(alerts); }

  acknowledgeAlert(id: string): void {
    this._alerts.set(this._alerts().map((a) => a.id === id ? { ...a, acknowledged: true } : a));
  }

  updateActiveUsers(data: ActiveUsersStat): void { this._activeUsers.set(data); }

  setConnected(b: boolean): void { this._connected.set(b); }

  // --------------------------------------------------------- bootstrap (mock)
  /** Charge un snapshot mock pour le dev. En prod, remplacer par HTTP ou SSE. */
  loadMockSnapshot(): void {
    if (!isPlatformBrowser(this.platformId)) return;
    this.setServices(MOCK_SERVICES);
    this.setAlerts(MOCK_ALERTS);
    this.updateActiveUsers({
      total: 1412,
      byRole: { ELEVEUR: 247, CLIENT: 1102, PRODUCTEUR: 58, ADMIN: 5 },
    });
    this.setConnected(true);
  }
}

const MOCK_SERVICES: ServiceHealth[] = [
  { name: 'ARMAGEDDON gateway', category: 'gateway', status: 'UP',       latencyP99Ms: 82,  requestsPerSec: 412, errorRate: 0.02, uptime: '99.98%', lastCheck: new Date().toISOString() },
  { name: 'KAYA cache',         category: 'cache',   status: 'UP',       latencyP99Ms: 3,   requestsPerSec: 1880, errorRate: 0.00, uptime: '99.99%', lastCheck: new Date().toISOString() },
  { name: 'ORY Kratos',         category: 'auth',    status: 'DEGRADED', latencyP99Ms: 420, requestsPerSec: 48,  errorRate: 0.18, uptime: '99.85%', lastCheck: new Date().toISOString() },
  { name: 'auth-ms',            category: 'app',     status: 'UP',       latencyP99Ms: 95,  requestsPerSec: 62,  errorRate: 0.01, uptime: '99.96%', lastCheck: new Date().toISOString() },
  { name: 'poulets-bff',        category: 'app',     status: 'UP',       latencyP99Ms: 140, requestsPerSec: 205, errorRate: 0.03, uptime: '99.94%', lastCheck: new Date().toISOString() },
  { name: 'Postgres',           category: 'db',      status: 'UP',       latencyP99Ms: 12,  requestsPerSec: 312, errorRate: 0.00, uptime: '100%',   lastCheck: new Date().toISOString() },
  { name: 'RedPanda (outbox)',  category: 'broker',  status: 'UP',       latencyP99Ms: 4,   requestsPerSec: 640, errorRate: 0.00, uptime: '99.97%', lastCheck: new Date().toISOString() },
];

const MOCK_ALERTS: SystemAlert[] = [
  {
    id: 'a1', type: 'LatencyHigh', severity: 'warning', service: 'ORY Kratos',
    message: 'p95 login latency > 400ms for 10 min',
    createdAt: new Date(Date.now() - 22 * 60000).toISOString(), acknowledged: false,
  },
  {
    id: 'a2', type: 'CertExpiring', severity: 'info', service: 'ARMAGEDDON',
    message: 'SPIFFE SVID expires in 68h',
    createdAt: new Date(Date.now() - 4 * 3600000).toISOString(), acknowledged: false,
  },
  {
    id: 'a3', type: 'DiskUsage', severity: 'warning', service: 'Postgres',
    message: 'Volume data 78% used',
    createdAt: new Date(Date.now() - 12 * 3600000).toISOString(), acknowledged: true,
    acknowledgedBy: 'admin@fasodigitalisation.bf',
  },
];
