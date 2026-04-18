// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, OnInit, inject, signal } from '@angular/core';
import { CommonModule, DatePipe } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';
import { MatSnackBar } from '@angular/material/snack-bar';

import { LoadingComponent } from '@shared/components/loading/loading.component';

interface SessionRow {
  id: string;
  device: string;
  browser: string;
  ip: string;
  location?: string;
  lastActive: string;
  current: boolean;
}

@Component({
  selector: 'app-security-sessions',
  standalone: true,
  imports: [CommonModule, DatePipe, RouterLink, MatIconModule, MatButtonModule, LoadingComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="page">
      <header>
        <div>
          <h1>Sessions actives</h1>
          <p>Tous les appareils connectés à votre compte</p>
        </div>
        <button mat-stroked-button type="button" (click)="logoutOthers()" [disabled]="sessions().length <= 1">
          <mat-icon>logout</mat-icon>
          Déconnecter les autres sessions
        </button>
      </header>

      @if (loading()) {
        <app-loading message="Chargement des sessions…" />
      } @else {
        <ul class="sessions">
          @for (s of sessions(); track s.id) {
            <li [class.current]="s.current">
              <mat-icon>{{ deviceIcon(s.device) }}</mat-icon>
              <div>
                <strong>
                  {{ s.device }} · {{ s.browser }}
                  @if (s.current) { <span class="tag">Session actuelle</span> }
                </strong>
                <small>
                  {{ s.ip }}
                  @if (s.location) { · {{ s.location }} }
                  · Dernière activité {{ s.lastActive | date:'short' }}
                </small>
              </div>
              @if (!s.current) {
                <button mat-icon-button type="button" (click)="revoke(s)" aria-label="Déconnecter cette session">
                  <mat-icon>logout</mat-icon>
                </button>
              }
            </li>
          }
        </ul>
      }

      <footer class="foot">
        <a mat-button routerLink="/profile/mfa">
          <mat-icon>shield_lock</mat-icon>
          Configuration MFA
        </a>
        <a mat-button routerLink="/profile">
          <mat-icon>person</mat-icon>
          Retour au profil
        </a>
      </footer>
    </section>
  `,
  styles: [`
    :host { display: block; background: var(--faso-bg); min-height: 100vh; }
    :host > .page {
      max-width: 900px;
      margin: 0 auto;
      padding: var(--faso-space-6) var(--faso-space-4) var(--faso-space-12);
    }

    header {
      display: flex;
      justify-content: space-between;
      align-items: flex-end;
      gap: var(--faso-space-3);
      margin-bottom: var(--faso-space-5);
      flex-wrap: wrap;
    }
    header h1 { margin: 0; font-size: var(--faso-text-2xl); font-weight: var(--faso-weight-bold); }
    header p { margin: 4px 0 0; color: var(--faso-text-muted); }

    .sessions {
      list-style: none;
      padding: 0;
      margin: 0;
      display: flex;
      flex-direction: column;
      gap: var(--faso-space-2);
    }
    .sessions li {
      display: grid;
      grid-template-columns: auto 1fr auto;
      gap: var(--faso-space-3);
      align-items: center;
      padding: var(--faso-space-4);
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-lg);
    }
    .sessions li.current {
      border-color: var(--faso-primary-500);
      background: var(--faso-primary-50);
    }
    .sessions li > mat-icon {
      color: var(--faso-primary-700);
      font-size: 28px; width: 28px; height: 28px;
    }
    .sessions strong { display: block; }
    .sessions small { color: var(--faso-text-muted); font-size: var(--faso-text-xs); }
    .tag {
      display: inline-block;
      margin-left: 6px;
      padding: 1px 8px;
      background: var(--faso-primary-600);
      color: #FFFFFF;
      border-radius: var(--faso-radius-pill);
      font-size: var(--faso-text-xs);
      font-weight: var(--faso-weight-semibold);
      vertical-align: middle;
    }

    .foot {
      display: flex;
      gap: var(--faso-space-2);
      margin-top: var(--faso-space-6);
      padding-top: var(--faso-space-4);
      border-top: 1px solid var(--faso-border);
    }
  `],
})
export class SecuritySessionsComponent implements OnInit {
  private readonly snack = inject(MatSnackBar);

  readonly sessions = signal<SessionRow[]>([]);
  readonly loading = signal(true);

  ngOnInit(): void {
    // Stub. En prod : GET Kratos `/sessions` + parse User-Agent.
    setTimeout(() => {
      this.sessions.set([
        { id: 'curr', device: 'Smartphone', browser: 'Chrome Android', ip: '192.168.1.52',  location: 'Ouagadougou', lastActive: new Date().toISOString(), current: true },
        { id: 's2',   device: 'PC',         browser: 'Firefox 128',   ip: '10.0.12.4',      location: 'Ouagadougou', lastActive: new Date(Date.now() - 3600000).toISOString(), current: false },
        { id: 's3',   device: 'MacBook',    browser: 'Safari 17.3',   ip: '41.77.142.18',   location: 'Bobo-Dioulasso', lastActive: new Date(Date.now() - 48 * 3600000).toISOString(), current: false },
      ]);
      this.loading.set(false);
    }, 200);
  }

  deviceIcon(device: string): string {
    if (device.toLowerCase().includes('phone') || device.toLowerCase().includes('smartphone')) return 'smartphone';
    if (device.toLowerCase().includes('mac') || device.toLowerCase().includes('pc')) return 'laptop';
    if (device.toLowerCase().includes('tablet')) return 'tablet_mac';
    return 'computer';
  }

  revoke(s: SessionRow): void {
    this.sessions.update((arr) => arr.filter((x) => x.id !== s.id));
    this.snack.open(`Session ${s.device} déconnectée`, 'OK', { duration: 2500 });
  }

  logoutOthers(): void {
    this.sessions.update((arr) => arr.filter((x) => x.current));
    this.snack.open('Toutes les autres sessions ont été déconnectées', 'OK', { duration: 3000 });
  }
}
