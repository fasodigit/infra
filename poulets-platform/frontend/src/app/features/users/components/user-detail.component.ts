// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, OnInit, inject, signal } from '@angular/core';
import { CommonModule, DatePipe } from '@angular/common';
import { ActivatedRoute, Router, RouterLink } from '@angular/router';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';
import { MatSnackBar } from '@angular/material/snack-bar';

import { LoadingComponent } from '@shared/components/loading/loading.component';
import { SectionHeaderComponent } from '@shared/components/section-header/section-header.component';
import { PlatformUser } from '@shared/models/admin.models';
import { UsersService } from '../services/users.service';

@Component({
  selector: 'app-user-detail',
  standalone: true,
  imports: [
    CommonModule, DatePipe, RouterLink, MatIconModule, MatButtonModule,
    LoadingComponent, SectionHeaderComponent,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="page">
      <a mat-button routerLink="/admin/users" class="back">
        <mat-icon>arrow_back</mat-icon> Retour
      </a>

      @if (loading()) {
        <app-loading message="Chargement…" />
      } @else if (user(); as u) {
        <header class="head">
          <div class="identity">
            <span class="initials">{{ initials(u) }}</span>
            <div>
              <h1>{{ u.displayName }}</h1>
              <p>{{ u.email }} · {{ u.phone ?? '—' }}</p>
              <span class="role-badge" [class]="'role--' + u.role.toLowerCase()">{{ u.role }}</span>
              @if (u.region) { <span class="region">{{ u.region }}</span> }
            </div>
          </div>
          <div class="actions">
            <a mat-stroked-button [routerLink]="['edit']"><mat-icon>edit</mat-icon> Modifier</a>
            @if (u.isActive) {
              <button mat-stroked-button color="warn" type="button" (click)="deactivate(u)">
                <mat-icon>block</mat-icon> Désactiver
              </button>
            } @else {
              <button mat-stroked-button type="button" (click)="reactivate(u)">
                <mat-icon>check_circle</mat-icon> Réactiver
              </button>
            }
            <button mat-stroked-button type="button" (click)="forceLogout(u)">
              <mat-icon>logout</mat-icon> Forcer déconnexion
            </button>
          </div>
        </header>

        <div class="grid">
          <article class="card">
            <app-section-header title="Sécurité MFA" kicker="Méthodes configurées" />
            <ul class="mfa">
              <li [class.on]="u.mfaStatus.email">
                <mat-icon>mail</mat-icon>
                <span>
                  <strong>Email vérifié</strong>
                  <small>{{ u.email }}</small>
                </span>
                <span class="state">{{ u.mfaStatus.email ? '✓' : '—' }}</span>
              </li>
              <li [class.on]="u.mfaStatus.passkey">
                <mat-icon>fingerprint</mat-icon>
                <span>
                  <strong>PassKey / WebAuthn</strong>
                  <small>Empreinte, Face ID, clé USB</small>
                </span>
                <span class="state">{{ u.mfaStatus.passkey ? '✓' : '—' }}</span>
              </li>
              <li [class.on]="u.mfaStatus.totp">
                <mat-icon>qr_code_2</mat-icon>
                <span>
                  <strong>Google Authenticator (TOTP)</strong>
                  <small>Code 6 chiffres renouvelé toutes les 30s</small>
                </span>
                <span class="state">{{ u.mfaStatus.totp ? '✓' : '—' }}</span>
              </li>
              <li [class.on]="u.mfaStatus.backupCodes">
                <mat-icon>vpn_key</mat-icon>
                <span>
                  <strong>Codes de secours</strong>
                  <small>10 codes à usage unique</small>
                </span>
                <span class="state">{{ u.mfaStatus.backupCodes ? '✓' : '—' }}</span>
              </li>
              <li [class.on]="u.mfaStatus.phone">
                <mat-icon>sms</mat-icon>
                <span>
                  <strong>SMS</strong>
                  <small>{{ u.phone ?? 'Non renseigné' }}</small>
                </span>
                <span class="state">{{ u.mfaStatus.phone ? '✓' : '—' }}</span>
              </li>
            </ul>
          </article>

          <article class="card">
            <app-section-header title="Activité" />
            <dl>
              <div>
                <dt>Statut</dt>
                <dd>
                  <span class="status" [class.active]="u.isActive">
                    <span class="dot"></span>
                    {{ u.isActive ? 'Actif' : 'Inactif' }}
                  </span>
                </dd>
              </div>
              <div><dt>Créé le</dt><dd>{{ u.createdAt | date:'fullDate' }}</dd></div>
              <div><dt>Dernière connexion</dt><dd>{{ u.lastLoginAt ? (u.lastLoginAt | date:'medium') : '—' }}</dd></div>
              <div><dt>Téléphone</dt><dd>{{ u.phone ?? '—' }}</dd></div>
              <div><dt>Région</dt><dd>{{ u.region ?? '—' }}</dd></div>
            </dl>
          </article>

          @if (u.roleMeta) {
            <article class="card full">
              <app-section-header [title]="'Configuration ' + u.role" />
              <pre class="meta">{{ u.roleMeta | json }}</pre>
            </article>
          }
        </div>
      } @else {
        <p class="empty">Utilisateur introuvable.</p>
      }
    </section>
  `,
  styles: [`
    :host { display: block; }
    .back { margin-left: calc(var(--faso-space-4) * -1); color: var(--faso-text-muted); }

    .head {
      display: flex;
      justify-content: space-between;
      align-items: flex-start;
      gap: var(--faso-space-4);
      margin: var(--faso-space-2) 0 var(--faso-space-6);
      flex-wrap: wrap;
    }
    .identity {
      display: flex;
      align-items: center;
      gap: var(--faso-space-4);
    }
    .initials {
      width: 64px; height: 64px;
      border-radius: 50%;
      background: var(--faso-primary-100);
      color: var(--faso-primary-700);
      display: inline-flex;
      align-items: center;
      justify-content: center;
      font-size: 1.4rem;
      font-weight: var(--faso-weight-semibold);
    }
    .identity h1 { margin: 0; font-size: var(--faso-text-2xl); font-weight: var(--faso-weight-bold); }
    .identity p { margin: 4px 0 8px; color: var(--faso-text-muted); }
    .role-badge {
      display: inline-flex;
      padding: 2px 10px;
      border-radius: var(--faso-radius-pill);
      font-size: var(--faso-text-xs);
      font-weight: var(--faso-weight-semibold);
      margin-right: 8px;
    }
    .role--eleveur    { background: var(--faso-primary-50);  color: var(--faso-primary-700); }
    .role--client     { background: var(--faso-info-bg);     color: var(--faso-info); }
    .role--producteur { background: var(--faso-accent-100);  color: var(--faso-accent-800); }
    .role--admin      { background: var(--faso-warning-bg);  color: var(--faso-warning); }
    .region { color: var(--faso-text-muted); font-size: var(--faso-text-sm); }

    .actions { display: flex; gap: var(--faso-space-2); flex-wrap: wrap; }

    .grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(360px, 1fr));
      gap: var(--faso-space-4);
    }
    .card {
      padding: var(--faso-space-5);
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
    }
    .card.full { grid-column: 1 / -1; }

    .mfa {
      list-style: none;
      padding: 0;
      margin: 0;
      display: flex;
      flex-direction: column;
      gap: var(--faso-space-2);
    }
    .mfa li {
      display: grid;
      grid-template-columns: auto 1fr auto;
      align-items: center;
      gap: var(--faso-space-3);
      padding: var(--faso-space-3);
      background: var(--faso-surface-alt);
      border-radius: var(--faso-radius-md);
      color: var(--faso-text-muted);
    }
    .mfa li.on {
      background: var(--faso-success-bg);
      color: var(--faso-text);
    }
    .mfa li strong { display: block; }
    .mfa li small { color: var(--faso-text-muted); }
    .mfa .state { font-weight: var(--faso-weight-bold); color: var(--faso-success); }
    .mfa li:not(.on) .state { color: var(--faso-text-subtle); }

    dl { margin: 0; display: flex; flex-direction: column; gap: 8px; }
    dl div { display: flex; justify-content: space-between; padding: 6px 0; border-bottom: 1px solid var(--faso-border); }
    dl div:last-child { border-bottom: none; }
    dt { color: var(--faso-text-muted); font-size: var(--faso-text-sm); }
    dd { margin: 0; font-weight: var(--faso-weight-medium); }

    .status {
      display: inline-flex; align-items: center; gap: 4px;
      font-size: var(--faso-text-sm);
      color: var(--faso-text-muted);
    }
    .status .dot { width: 8px; height: 8px; border-radius: 50%; background: var(--faso-text-subtle); }
    .status.active { color: var(--faso-success); }
    .status.active .dot { background: var(--faso-success); box-shadow: 0 0 0 3px var(--faso-success-bg); }

    .meta {
      background: var(--faso-surface-alt);
      padding: var(--faso-space-3);
      border-radius: var(--faso-radius-md);
      font-family: var(--faso-font-mono);
      font-size: var(--faso-text-sm);
      overflow-x: auto;
      margin: 0;
    }

    .empty { padding: var(--faso-space-10); text-align: center; color: var(--faso-text-muted); }
  `],
})
export class UserDetailComponent implements OnInit {
  private readonly route = inject(ActivatedRoute);
  private readonly router = inject(Router);
  private readonly svc = inject(UsersService);
  private readonly snack = inject(MatSnackBar);

  readonly user = signal<PlatformUser | null>(null);
  readonly loading = signal(true);

  ngOnInit(): void {
    const id = this.route.snapshot.paramMap.get('id');
    if (!id) return;
    this.svc.get(id).subscribe({
      next: (u) => { this.user.set(u); this.loading.set(false); },
      error: () => this.loading.set(false),
    });
  }

  initials(u: PlatformUser): string {
    return [u.firstName, u.lastName].filter(Boolean).map((n) => n!.charAt(0).toUpperCase()).join('');
  }

  deactivate(u: PlatformUser): void {
    this.svc.deactivate(u.id).subscribe((next) => {
      this.user.set(next);
      this.snack.open('Utilisateur désactivé', 'OK', { duration: 2500 });
    });
  }
  reactivate(u: PlatformUser): void {
    this.svc.reactivate(u.id).subscribe((next) => {
      this.user.set(next);
      this.snack.open('Utilisateur réactivé', 'OK', { duration: 2500 });
    });
  }
  forceLogout(u: PlatformUser): void {
    this.svc.forceLogout(u.id).subscribe(() => {
      this.snack.open('Sessions Kratos invalidées', 'OK', { duration: 2500 });
    });
  }
}
