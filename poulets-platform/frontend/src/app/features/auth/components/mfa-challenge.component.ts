// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { ActivatedRoute, Router } from '@angular/router';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';
import { MatTabsModule } from '@angular/material/tabs';
import { MatSnackBar } from '@angular/material/snack-bar';

import { KratosSettingsService } from '@core/kratos/kratos-settings.service';

type MfaMethod = 'passkey' | 'totp' | 'lookup' | 'sms';

@Component({
  selector: 'app-mfa-challenge',
  standalone: true,
  imports: [CommonModule, FormsModule, MatIconModule, MatButtonModule, MatTabsModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="page">
      <div class="container">
        <header>
          <mat-icon>shield_lock</mat-icon>
          <h1>Vérification en 2 étapes</h1>
          <p>Pour continuer, confirmez votre identité avec l'une des méthodes ci-dessous.</p>
        </header>

        <mat-tab-group mat-stretch-tabs="false" animationDuration="200ms">
          <mat-tab label="PassKey">
            <ng-template mat-tab-label>
              <mat-icon>fingerprint</mat-icon>
              <span>PassKey</span>
            </ng-template>
            <div class="tab">
              <p>Utilisez votre empreinte digitale, Face ID ou votre clé de sécurité USB.</p>
              <button mat-raised-button color="primary" type="button" (click)="challengePasskey()" [disabled]="busy()">
                <mat-icon>fingerprint</mat-icon>
                @if (busy()) { Vérification… } @else { Utiliser ma PassKey }
              </button>
              <p class="hint">Recommandé · le plus sécurisé</p>
            </div>
          </mat-tab>

          <mat-tab label="TOTP">
            <ng-template mat-tab-label>
              <mat-icon>qr_code_2</mat-icon>
              <span>Code TOTP</span>
            </ng-template>
            <div class="tab">
              <p>Ouvrez votre application Authenticator (Google Authenticator, Authy…) et saisissez le code à 6 chiffres.</p>
              <input
                [(ngModel)]="totpCode"
                type="text"
                inputmode="numeric"
                pattern="\\d{6}"
                maxlength="6"
                placeholder="000 000"
                class="code-input"
                aria-label="Code TOTP à 6 chiffres"
              >
              <button mat-raised-button color="primary" type="button"
                      (click)="challengeTotp()"
                      [disabled]="busy() || totpCode.length !== 6">
                @if (busy()) { Vérification… } @else { Valider }
              </button>
            </div>
          </mat-tab>

          <mat-tab label="Code de secours">
            <ng-template mat-tab-label>
              <mat-icon>vpn_key</mat-icon>
              <span>Code secours</span>
            </ng-template>
            <div class="tab">
              <p>Utilisez un de vos 10 codes de secours (à usage unique).</p>
              <input
                [(ngModel)]="lookupCode"
                type="text"
                placeholder="ABCD-1234"
                class="code-input"
                style="letter-spacing: 0.15em;"
                aria-label="Code de secours"
              >
              <button mat-raised-button color="primary" type="button"
                      (click)="challengeLookup()"
                      [disabled]="busy() || !lookupCode">
                @if (busy()) { Vérification… } @else { Valider }
              </button>
            </div>
          </mat-tab>
        </mat-tab-group>

        <footer>
          <button mat-button type="button" (click)="cancel()">
            <mat-icon>arrow_back</mat-icon> Annuler
          </button>
          <span>Besoin d'aide ? <a href="mailto:support@fasodigitalisation.bf">Contactez le support</a></span>
        </footer>
      </div>
    </section>
  `,
  styles: [`
    :host { display: block; background: #F9FAFB; min-height: 100vh; color: #0F172A; }
    .container {
      max-width: 520px;
      margin: 0 auto;
      padding: var(--faso-space-10) var(--faso-space-4);
    }
    header { text-align: center; margin-bottom: var(--faso-space-6); }
    header mat-icon {
      font-size: 56px; width: 56px; height: 56px;
      color: #2E7D32;
      margin-bottom: var(--faso-space-2);
    }
    header h1 { margin: 0; font-size: 1.5rem; font-weight: 700; color: #0F172A; }
    header p { margin: 4px 0 0; color: #475569; max-width: 42ch; margin-inline: auto; }

    mat-tab-group {
      background: #FFFFFF;
      border: 1px solid #E5E7EB;
      border-radius: 12px;
      overflow: hidden;
    }
    .tab {
      display: flex;
      flex-direction: column;
      align-items: flex-start;
      gap: var(--faso-space-3);
      padding: var(--faso-space-5);
      color: #0F172A;
    }
    .tab p { margin: 0; color: #475569; }
    .tab .hint {
      color: #2E7D32;
      font-weight: 600;
      font-size: 0.875rem;
    }

    .code-input {
      width: 100%;
      padding: 12px 16px;
      border: 1px solid #D1D5DB;
      border-radius: 8px;
      font-family: var(--faso-font-mono);
      font-size: 1.25rem;
      text-align: center;
      letter-spacing: 0.25em;
      background: #FFFFFF;
      color: #0F172A;
    }
    .code-input:focus {
      outline: none;
      border-color: #2E7D32;
      box-shadow: 0 0 0 3px rgba(46, 125, 50, 0.18);
    }

    footer {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-top: var(--faso-space-5);
      color: #475569;
      font-size: 0.875rem;
      flex-wrap: wrap;
      gap: var(--faso-space-2);
    }
    footer a { color: #1B5E20; }
  `],
})
export class MfaChallengeComponent {
  private readonly route = inject(ActivatedRoute);
  private readonly router = inject(Router);
  private readonly snack = inject(MatSnackBar);
  private readonly kratos = inject(KratosSettingsService);

  readonly busy = signal(false);
  totpCode = '';
  lookupCode = '';

  async challengePasskey() {
    if (!this.kratos.isBrowser) return;
    this.busy.set(true);
    try {
      const { startAuthentication } = await import('@simplewebauthn/browser');
      const options: any = {
        challenge: btoa(crypto.getRandomValues(new Uint8Array(32)).reduce((s, b) => s + String.fromCharCode(b), '')),
        rpId: window.location.hostname,
        timeout: 60000,
        userVerification: 'preferred',
        allowCredentials: [],
      };
      await startAuthentication({ optionsJSON: options });
      this.snack.open('PassKey validée', 'OK', { duration: 2500 });
      this.redirect();
    } catch (err: any) {
      this.snack.open('PassKey échouée : ' + (err?.message ?? 'annulée'), 'OK', { duration: 4000 });
    } finally {
      this.busy.set(false);
    }
  }

  challengeTotp(): void {
    this.busy.set(true);
    this.kratos.verifyTotp(this.totpCode).subscribe((ok) => {
      this.busy.set(false);
      if (ok) {
        this.snack.open('Code vérifié', 'OK', { duration: 2500 });
        this.redirect();
      } else {
        this.snack.open('Code TOTP invalide', 'OK', { duration: 3000 });
      }
    });
  }

  challengeLookup(): void {
    this.busy.set(true);
    // Stub : en prod, POST au flow login avec `lookup_secret` node.
    setTimeout(() => {
      this.busy.set(false);
      this.snack.open('Code de secours validé (stub)', 'OK', { duration: 2500 });
      this.redirect();
    }, 400);
  }

  cancel(): void {
    this.router.navigate(['/']);
  }

  private redirect(): void {
    const returnTo = this.route.snapshot.queryParamMap.get('return_to') ?? '/dashboard';
    this.router.navigateByUrl(returnTo);
  }
}
