// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

/**
 * FasoApprovalModalComponent — Push-approval number-matching modal.
 *
 * Phase 4.b.5 — sovereign WebSocket MFA, anti-MFA-bombing.
 *
 * ## Number-matching UX
 * - Onglet login (Tab 1) affiche un grand chiffre unique (ex : "07").
 * - Modal (Tab 2 / device approuvé) affiche 3 grands boutons chiffres, dont
 *   le bon. L'utilisateur doit taper LE bon chiffre.
 * - Si le chiffre est mauvais : DENIED + audit `PUSH_APPROVAL_NUMBER_MISMATCH`.
 * - Timeout 30 s : countdown visible + fallback automatique vers OTP.
 *
 * ## Inputs
 * - `request` : l'`ApprovalRequest` reçu du WS (numbers, ip, ua, city, expiresAt).
 * - `(approved)` : émet `ApprovalResult` quand l'utilisateur valide.
 * - `(fallback)` : émet `void` quand le délai expire ou user clique "Utiliser OTP".
 *
 * ## Accessibilité
 * - Rôle `dialog`, aria-modal, aria-labelledby.
 * - Boutons chiffres ≥ 64x64 px, police mono taille 2rem.
 * - Countdown annoncé via aria-live="polite".
 */

import {
  ChangeDetectionStrategy,
  Component,
  EventEmitter,
  Input,
  OnChanges,
  OnDestroy,
  OnInit,
  Output,
  SimpleChanges,
  inject,
  signal,
} from '@angular/core';
import { CommonModule } from '@angular/common';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatProgressBarModule } from '@angular/material/progress-bar';
import { Subject, interval, takeUntil } from 'rxjs';

import {
  ApprovalRequest,
  ApprovalResult,
  PushApprovalService,
} from '../services/push-approval.service';

@Component({
  selector: 'faso-approval-modal',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [CommonModule, MatButtonModule, MatIconModule, MatProgressBarModule],
  template: `
    @if (request) {
      <div
        class="overlay"
        role="dialog"
        aria-modal="true"
        [attr.aria-labelledby]="'approval-title-' + request.requestId"
      >
        <div class="card" data-testid="push-approval-modal">

          <!-- Header -->
          <div class="card-header">
            <mat-icon class="header-icon">shield_lock</mat-icon>
            <h2 [id]="'approval-title-' + request.requestId">
              Demande de connexion
            </h2>
          </div>

          <!-- Login context -->
          <div class="context-block">
            <div class="context-row">
              <mat-icon>location_on</mat-icon>
              <span>{{ request.city || 'Lieu inconnu' }} · {{ request.ip }}</span>
            </div>
            <div class="context-row">
              <mat-icon>computer</mat-icon>
              <span>{{ uaShort() }}</span>
            </div>
          </div>

          <!-- Number-matching instruction -->
          <p class="instruction">
            Tapez le chiffre affiché sur l'écran de connexion :
          </p>
          <p class="displayed-hint" aria-label="Indice : regardez l'onglet de connexion">
            Le bon chiffre est affiché sur l'autre onglet
          </p>

          <!-- 3 number buttons -->
          <div class="number-grid" role="group" aria-label="Choisissez le bon chiffre">
            @for (n of request.numbers; track n) {
              <button
                mat-flat-button
                class="number-btn"
                [class.selected]="selectedNumber() === n"
                [disabled]="busy() || answered()"
                (click)="selectNumber(n)"
                [attr.data-testid]="'number-btn-' + n"
                [attr.aria-pressed]="selectedNumber() === n"
              >
                {{ n.toString().padStart(2, '0') }}
              </button>
            }
          </div>

          <!-- Result banner -->
          @if (resultMessage()) {
            <div
              class="result-banner"
              [class.granted]="lastResult()?.granted"
              [class.denied]="!lastResult()?.granted"
              role="status"
              aria-live="assertive"
            >
              <mat-icon>{{ lastResult()?.granted ? 'check_circle' : 'cancel' }}</mat-icon>
              <span>{{ resultMessage() }}</span>
            </div>
          }

          <!-- Timeout countdown -->
          <div class="timeout-bar" aria-live="polite" [attr.aria-label]="'Expiration dans ' + secondsLeft() + ' secondes'">
            <mat-progress-bar
              mode="determinate"
              [value]="progressValue()"
              [color]="secondsLeft() <= 5 ? 'warn' : 'primary'"
            ></mat-progress-bar>
            <span class="countdown" [class.urgent]="secondsLeft() <= 5">
              {{ secondsLeft() }} s
            </span>
          </div>

          <!-- Fallback link -->
          <div class="footer-actions">
            <button
              mat-button
              class="fallback-btn"
              (click)="triggerFallback()"
              [disabled]="busy()"
              data-testid="push-approval-fallback"
            >
              <mat-icon>sms</mat-icon>
              Utiliser le code OTP à la place
            </button>
          </div>

        </div>
      </div>
    }
  `,
  styles: [`
    :host { display: block; }

    .overlay {
      position: fixed;
      inset: 0;
      background: rgba(0, 0, 0, 0.65);
      display: flex;
      align-items: center;
      justify-content: center;
      z-index: 9000;
      padding: 16px;
    }

    .card {
      background: #FFFFFF;
      border-radius: 16px;
      padding: 32px 28px 24px;
      width: 100%;
      max-width: 440px;
      box-shadow: 0 24px 60px rgba(0, 0, 0, 0.30);
      color: #0F172A;
    }

    .card-header {
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: 8px;
      margin-bottom: 20px;
      text-align: center;
    }
    .header-icon {
      font-size: 48px;
      width: 48px;
      height: 48px;
      color: #D97706;
    }
    .card-header h2 {
      margin: 0;
      font-size: 1.25rem;
      font-weight: 700;
      color: #0F172A;
    }

    .context-block {
      background: #F8FAFC;
      border: 1px solid #E5E7EB;
      border-radius: 10px;
      padding: 12px 14px;
      margin-bottom: 20px;
      display: flex;
      flex-direction: column;
      gap: 8px;
    }
    .context-row {
      display: flex;
      align-items: center;
      gap: 8px;
      font-size: 0.875rem;
      color: #475569;
    }
    .context-row mat-icon {
      font-size: 18px;
      width: 18px;
      height: 18px;
      color: #64748B;
      flex-shrink: 0;
    }

    .instruction {
      margin: 0 0 4px;
      font-size: 0.9375rem;
      font-weight: 600;
      color: #0F172A;
      text-align: center;
    }
    .displayed-hint {
      margin: 0 0 20px;
      font-size: 0.8125rem;
      color: #64748B;
      text-align: center;
    }

    .number-grid {
      display: grid;
      grid-template-columns: repeat(3, 1fr);
      gap: 14px;
      margin-bottom: 20px;
    }

    .number-btn {
      height: 72px !important;
      font-size: 2rem !important;
      font-weight: 700 !important;
      font-family: var(--faso-font-mono, 'Courier New', monospace) !important;
      border-radius: 12px !important;
      letter-spacing: 0.05em;
      background: #F1F5F9 !important;
      color: #0F172A !important;
      border: 2px solid #E2E8F0 !important;
      transition: background 120ms, border-color 120ms, transform 80ms;
    }
    .number-btn:hover:not([disabled]) {
      background: #DBEAFE !important;
      border-color: #3B82F6 !important;
    }
    .number-btn.selected {
      background: #2E7D32 !important;
      border-color: #1B5E20 !important;
      color: #FFFFFF !important;
      transform: scale(1.04);
    }
    .number-btn[disabled] { opacity: 0.45; }

    .result-banner {
      display: flex;
      align-items: center;
      gap: 8px;
      padding: 10px 14px;
      border-radius: 8px;
      font-size: 0.875rem;
      font-weight: 600;
      margin-bottom: 16px;
    }
    .result-banner.granted {
      background: #DCFCE7;
      color: #15803D;
      border: 1px solid #86EFAC;
    }
    .result-banner.denied {
      background: #FEE2E2;
      color: #DC2626;
      border: 1px solid #FCA5A5;
    }
    .result-banner mat-icon { font-size: 20px; width: 20px; height: 20px; }

    .timeout-bar {
      display: flex;
      align-items: center;
      gap: 12px;
      margin-bottom: 16px;
    }
    .timeout-bar mat-progress-bar { flex: 1; }
    .countdown {
      font-size: 0.875rem;
      font-weight: 600;
      color: #64748B;
      min-width: 32px;
      text-align: right;
    }
    .countdown.urgent { color: #DC2626; }

    .footer-actions {
      display: flex;
      justify-content: center;
    }
    .fallback-btn {
      color: #64748B !important;
      font-size: 0.8125rem !important;
    }
    .fallback-btn:hover { color: #475569 !important; }
    .fallback-btn mat-icon { font-size: 16px; width: 16px; height: 16px; margin-right: 4px; }
  `],
})
export class FasoApprovalModalComponent implements OnInit, OnChanges, OnDestroy {
  private readonly pushApprovalSvc = inject(PushApprovalService);
  private readonly destroy$ = new Subject<void>();

  @Input() request: ApprovalRequest | null = null;
  @Output() readonly approved = new EventEmitter<ApprovalResult>();
  @Output() readonly fallback = new EventEmitter<void>();

  readonly busy = signal(false);
  readonly answered = signal(false);
  readonly selectedNumber = signal<number | null>(null);
  readonly secondsLeft = signal(30);
  readonly progressValue = signal(100);
  readonly resultMessage = signal('');
  readonly lastResult = signal<ApprovalResult | null>(null);

  private totalSeconds = 30;

  ngOnInit(): void {
    this.startCountdown();
  }

  ngOnChanges(changes: SimpleChanges): void {
    if (changes['request'] && this.request) {
      // Reset state when a new request arrives.
      this.answered.set(false);
      this.selectedNumber.set(null);
      this.busy.set(false);
      this.resultMessage.set('');
      this.lastResult.set(null);

      const now = Date.now();
      const remaining = Math.max(0, Math.ceil((this.request.expiresAt - now) / 1000));
      this.totalSeconds = remaining || 30;
      this.secondsLeft.set(this.totalSeconds);
      this.progressValue.set(100);

      this.destroy$.next(); // cancel previous countdown
      this.startCountdown();
    }
  }

  ngOnDestroy(): void {
    this.destroy$.next();
    this.destroy$.complete();
  }

  selectNumber(n: number): void {
    if (this.busy() || this.answered() || !this.request) return;
    this.selectedNumber.set(n);
    this.submitResponse(this.request.requestId, n);
  }

  triggerFallback(): void {
    this.fallback.emit();
  }

  uaShort(): string {
    if (!this.request?.ua) return 'Navigateur inconnu';
    const ua = this.request.ua;
    const browserMatch =
      ua.match(/Chrome\/(\d+)/)?.[0] ||
      ua.match(/Firefox\/(\d+)/)?.[0] ||
      ua.match(/Safari\/(\d+)/)?.[0] ||
      ua.match(/Edge\/(\d+)/)?.[0];
    return browserMatch ?? ua.substring(0, 50);
  }

  // ── private ────────────────────────────────────────────────────────────────

  private startCountdown(): void {
    interval(1000)
      .pipe(takeUntil(this.destroy$))
      .subscribe(() => {
        const current = this.secondsLeft();
        if (current <= 1) {
          this.secondsLeft.set(0);
          this.progressValue.set(0);
          if (!this.answered()) {
            this.resultMessage.set('Délai expiré. Redirection vers le code OTP...');
            this.triggerFallback();
          }
          this.destroy$.next();
        } else {
          this.secondsLeft.set(current - 1);
          this.progressValue.set(Math.round(((current - 1) / this.totalSeconds) * 100));
        }
      });
  }

  private submitResponse(requestId: string, chosenNumber: number): void {
    this.busy.set(true);
    this.pushApprovalSvc.respond(requestId, chosenNumber)
      .pipe(takeUntil(this.destroy$))
      .subscribe({
        next: (result) => {
          this.busy.set(false);
          this.answered.set(true);
          this.lastResult.set(result);
          if (result.granted) {
            this.resultMessage.set('Connexion approuvée.');
            this.approved.emit(result);
          } else {
            this.resultMessage.set('Mauvais chiffre — connexion refusée.');
            // Trigger fallback after brief display.
            setTimeout(() => this.fallback.emit(), 1800);
          }
        },
        error: (err: unknown) => {
          this.busy.set(false);
          const msg = err instanceof Error ? err.message : 'Erreur réseau';
          this.resultMessage.set(`Erreur : ${msg}. Utilisation du code OTP.`);
          setTimeout(() => this.fallback.emit(), 1500);
        },
      });
  }
}
