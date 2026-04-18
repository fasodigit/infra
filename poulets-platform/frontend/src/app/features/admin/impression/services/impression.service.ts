// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { Injectable, inject, signal } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Observable, delay, of } from 'rxjs';
import { environment } from '@env/environment';

export type ImpressionType =
  | 'CERTIFICAT_HALAL'
  | 'CONTRAT_COMMANDE'
  | 'RECEPISSE_LIVRAISON'
  | 'ATTESTATION_ELEVAGE';

export type JobStatus = 'EN_ATTENTE' | 'EN_COURS' | 'TERMINE' | 'ECHOUE';

export interface ImpressionJob {
  id: string;
  type: ImpressionType;
  documentId: string;
  requestedBy: string;
  requestedAt: string;
  status: JobStatus;
  attempts: number;
  pdfUrl?: string;
  wormId?: string;
  qrCode?: string;
  errorMessage?: string;
  completedAt?: string;
}

export interface WormArchive {
  id: string;
  jobId: string;
  documentId: string;
  type: ImpressionType;
  sha256: string;
  blockchainTxId?: string;
  sealedAt: string;
  expiresAt?: string;
  qrVerificationUrl: string;
}

export interface RenderTemplate {
  name: string;
  label: string;
  description: string;
  variables: string[];
  updatedAt: string;
}

/**
 * Client HTTP vers `poulets-bff` → `impression-service` (port 8921) →
 * `ec-certificate-renderer` (port 8920).
 *
 * Les endpoints BFF attendus :
 *   POST /api/impression/generate      { type, documentId, data }
 *   GET  /api/impression/queue
 *   GET  /api/impression/:id
 *   GET  /api/impression/:id/pdf       → Blob application/pdf
 *   GET  /api/impression/archives
 *   GET  /api/impression/templates
 *   POST /api/verification/qr/:code    → { valid, sealedAt, jobId }
 *
 * En dev, les retours sont mockés — remplacer par `this.http.*` quand le
 * BFF est implémenté.
 */
@Injectable({ providedIn: 'root' })
export class ImpressionService {
  private readonly http = inject(HttpClient);
  private readonly api = ((environment as any).bffUrl ?? '/api') + '/impression';

  private readonly _jobs = signal<ImpressionJob[]>(mockJobs());
  private readonly _archives = signal<WormArchive[]>(mockArchives());
  readonly jobs = this._jobs.asReadonly();
  readonly archives = this._archives.asReadonly();

  generate(type: ImpressionType, documentId: string, data: Record<string, unknown>): Observable<ImpressionJob> {
    const job: ImpressionJob = {
      id: 'job-' + Math.random().toString(36).slice(2, 8),
      type,
      documentId,
      requestedBy: 'admin@fasodigitalisation.bf',
      requestedAt: new Date().toISOString(),
      status: 'EN_ATTENTE',
      attempts: 0,
    };
    this._jobs.update((arr) => [job, ...arr]);
    // Simulate async progress
    setTimeout(() => this.simulateProgress(job.id), 500);
    return of(job).pipe(delay(200));
  }

  listJobs(status?: JobStatus): Observable<ImpressionJob[]> {
    const arr = status ? this._jobs().filter((j) => j.status === status) : this._jobs();
    return of(arr).pipe(delay(100));
  }

  getJob(id: string): Observable<ImpressionJob | null> {
    return of(this._jobs().find((j) => j.id === id) ?? null).pipe(delay(80));
  }

  listArchives(): Observable<WormArchive[]> {
    return of(this._archives()).pipe(delay(120));
  }

  listTemplates(): Observable<RenderTemplate[]> {
    return of([
      { name: 'certificat-halal',    label: 'Certificat Halal',    description: 'Certif halal pour lot certifié',        variables: ['eleveurName', 'lotId', 'quantity', 'race', 'abattoir', 'sacrificateur', 'dateAbattage'], updatedAt: '2026-04-10T08:00:00Z' },
      { name: 'contrat-commande',    label: 'Contrat de commande', description: 'Accord client/éleveur',                  variables: ['clientName', 'eleveurName', 'orderId', 'quantity', 'amount', 'dateCommande'],              updatedAt: '2026-04-08T10:15:00Z' },
      { name: 'recepisse-livraison', label: 'Récépissé livraison', description: 'Bon de livraison signé',                variables: ['clientName', 'address', 'quantity', 'livreur', 'dateLivraison'],                          updatedAt: '2026-03-22T14:30:00Z' },
      { name: 'attestation-elevage', label: 'Attestation d\'élevage', description: 'Preuve d\'activité éleveur',         variables: ['eleveurName', 'region', 'memberSince', 'totalSales'],                                    updatedAt: '2026-03-15T09:00:00Z' },
    ]).pipe(delay(100));
  }

  verifyQr(code: string): Observable<{ valid: boolean; sealedAt?: string; jobId?: string }> {
    const archive = this._archives().find((a) => a.qrVerificationUrl.includes(code));
    return of(archive
      ? { valid: true, sealedAt: archive.sealedAt, jobId: archive.jobId }
      : { valid: false },
    ).pipe(delay(150));
  }

  downloadPdfUrl(jobId: string): string {
    return `${this.api}/${jobId}/pdf`;
  }

  // --------------------------------------------------------- mock simulation

  private simulateProgress(jobId: string) {
    this._jobs.update((arr) => arr.map((j) =>
      j.id === jobId ? { ...j, status: 'EN_COURS' as JobStatus, attempts: 1 } : j,
    ));
    setTimeout(() => {
      this._jobs.update((arr) => arr.map((j) => {
        if (j.id !== jobId) return j;
        const sealedAt = new Date().toISOString();
        const archive: WormArchive = {
          id: 'worm-' + Math.random().toString(36).slice(2, 8),
          jobId: j.id,
          documentId: j.documentId,
          type: j.type,
          sha256: Array.from({ length: 64 }, () => '0123456789abcdef'[Math.floor(Math.random() * 16)]).join(''),
          sealedAt,
          qrVerificationUrl: `https://poulets.fasodigitalisation.bf/verify/${j.id}`,
        };
        this._archives.update((a) => [archive, ...a]);
        return {
          ...j,
          status: 'TERMINE' as JobStatus,
          completedAt: sealedAt,
          wormId: archive.id,
          qrCode: archive.qrVerificationUrl,
          pdfUrl: this.downloadPdfUrl(j.id),
        };
      }));
    }, 1500);
  }
}

function mockJobs(): ImpressionJob[] {
  const now = Date.now();
  return [
    {
      id: 'job-001', type: 'CERTIFICAT_HALAL', documentId: 'L-2026-041',
      requestedBy: 'admin@fasodigitalisation.bf', requestedAt: new Date(now - 3600000).toISOString(),
      status: 'TERMINE', attempts: 1, completedAt: new Date(now - 3400000).toISOString(),
      wormId: 'worm-a1', qrCode: 'https://poulets.fasodigitalisation.bf/verify/job-001',
      pdfUrl: '/api/impression/job-001/pdf',
    },
    {
      id: 'job-002', type: 'CONTRAT_COMMANDE', documentId: 'CMD-A8X12',
      requestedBy: 'admin@fasodigitalisation.bf', requestedAt: new Date(now - 1800000).toISOString(),
      status: 'EN_COURS', attempts: 1,
    },
    {
      id: 'job-003', type: 'RECEPISSE_LIVRAISON', documentId: 'CMD-A8X11',
      requestedBy: 'livraison@poulets.bf', requestedAt: new Date(now - 900000).toISOString(),
      status: 'EN_ATTENTE', attempts: 0,
    },
    {
      id: 'job-004', type: 'ATTESTATION_ELEVAGE', documentId: 'u-1',
      requestedBy: 'admin@fasodigitalisation.bf', requestedAt: new Date(now - 7200000).toISOString(),
      status: 'ECHOUE', attempts: 3,
      errorMessage: 'Template render failed: missing variable `totalSales`',
    },
  ];
}

function mockArchives(): WormArchive[] {
  return [
    {
      id: 'worm-a1', jobId: 'job-001', documentId: 'L-2026-041', type: 'CERTIFICAT_HALAL',
      sha256: 'a8f3b2c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2',
      sealedAt: new Date(Date.now() - 3400000).toISOString(),
      qrVerificationUrl: 'https://poulets.fasodigitalisation.bf/verify/job-001',
    },
  ];
}
