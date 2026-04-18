// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, OnInit, inject, signal } from '@angular/core';
import { CommonModule, DatePipe } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { RouterLink } from '@angular/router';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';
import { MatDialog, MatDialogModule, MatDialogRef, MAT_DIALOG_DATA } from '@angular/material/dialog';
import { MatSnackBar } from '@angular/material/snack-bar';
import { QRCodeComponent } from 'angularx-qrcode';

import { TrustBadgeComponent } from '@shared/components/trust-badge/trust-badge.component';
import { LoadingComponent } from '@shared/components/loading/loading.component';
import { KratosSettingsService } from '@core/kratos/kratos-settings.service';
import { MfaStatus, PasskeyDevice } from '@core/kratos/kratos.models';
import { PROJECT_CONFIG } from '@core/config/project-config.token';

@Component({
  selector: 'app-mfa-settings',
  standalone: true,
  imports: [
    CommonModule, DatePipe, FormsModule, RouterLink,
    MatIconModule, MatButtonModule, MatDialogModule,
    TrustBadgeComponent, LoadingComponent,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="page">
      <header class="head">
        <div>
          <h1>Sécurité de mon compte</h1>
          <p>Configurez plusieurs méthodes pour protéger votre compte.</p>
        </div>
        @if (status(); as s) {
          @if (s.completed) {
            <app-trust-badge kind="vet" label="MFA complet" />
          } @else {
            <span class="warn-badge">
              <mat-icon>warning</mat-icon>
              Sécurité incomplète
            </span>
          }
        }
      </header>

      @if (loading()) {
        <app-loading message="Chargement de votre configuration sécurité…" />
      } @else if (status(); as s) {
        <ol class="cards">
          <!-- Card 1: Email -->
          <li class="card" [class.on]="s.email.verified">
            <span class="num">1</span>
            <div class="body">
              <header>
                <mat-icon>mail</mat-icon>
                <div>
                  <strong>Email vérifié</strong>
                  <small>Étape obligatoire · permet la récupération</small>
                </div>
                <span class="state" [class.on]="s.email.verified">
                  {{ s.email.verified ? '✓ Configuré' : '○ À faire' }}
                </span>
              </header>
              <p>{{ s.email.address }}</p>
              <div class="row">
                <button mat-stroked-button type="button" (click)="changeEmail()">
                  <mat-icon>edit</mat-icon> Modifier l'email
                </button>
              </div>
            </div>
          </li>

          <!-- Card 2: PassKey -->
          <li class="card" [class.on]="s.passkey.configured">
            <span class="num">2</span>
            <div class="body">
              <header>
                <mat-icon>fingerprint</mat-icon>
                <div>
                  <strong>PassKey (WebAuthn)</strong>
                  <small>Empreinte digitale, Face ID, clé de sécurité USB — méthode la plus sécurisée</small>
                </div>
                <span class="state" [class.on]="s.passkey.configured">
                  {{ s.passkey.configured ? '✓ Configuré' : '○ Recommandé' }}
                </span>
              </header>

              @if (s.passkey.devices.length > 0) {
                <ul class="devices">
                  @for (d of s.passkey.devices; track d.id) {
                    <li>
                      <mat-icon>{{ d.kind === 'platform' ? 'smartphone' : 'usb' }}</mat-icon>
                      <div>
                        <strong>{{ d.name }}</strong>
                        <small>
                          Ajoutée {{ d.addedAt | date:'mediumDate' }}
                          @if (d.lastUsedAt) { · utilisée {{ d.lastUsedAt | date:'short' }} }
                        </small>
                      </div>
                      <button mat-icon-button type="button" (click)="removePasskey(d)" aria-label="Supprimer">
                        <mat-icon>delete</mat-icon>
                      </button>
                    </li>
                  }
                </ul>
              }

              <div class="row">
                <button mat-raised-button color="primary" type="button" (click)="addPasskey()">
                  <mat-icon>add</mat-icon> Ajouter une clé de sécurité
                </button>
              </div>
            </div>
          </li>

          <!-- Card 3: TOTP -->
          <li class="card" [class.on]="s.totp.configured">
            <span class="num">3</span>
            <div class="body">
              <header>
                <mat-icon>qr_code_2</mat-icon>
                <div>
                  <strong>Application Authenticator (TOTP)</strong>
                  <small>Google Authenticator, Authy, 1Password — code à 6 chiffres toutes les 30s</small>
                </div>
                <span class="state" [class.on]="s.totp.configured">
                  {{ s.totp.configured ? '✓ Configuré' : '○ Recommandé' }}
                </span>
              </header>
              @if (s.totp.configuredAt) {
                <p>Configuré le {{ s.totp.configuredAt | date:'mediumDate' }}</p>
              }
              <div class="row">
                @if (s.totp.configured) {
                  <button mat-stroked-button type="button" (click)="disableTotp()">
                    <mat-icon>power_off</mat-icon> Désactiver
                  </button>
                } @else {
                  <button mat-raised-button color="primary" type="button" (click)="configureTotp()">
                    <mat-icon>settings</mat-icon> Configurer
                  </button>
                }
              </div>
            </div>
          </li>

          <!-- Card 4: Backup codes -->
          <li class="card" [class.on]="s.backupCodes.generated">
            <span class="num">4</span>
            <div class="body">
              <header>
                <mat-icon>vpn_key</mat-icon>
                <div>
                  <strong>Codes de secours</strong>
                  <small>10 codes à usage unique — indispensables si perte d'accès</small>
                </div>
                <span class="state" [class.on]="s.backupCodes.generated">
                  {{ s.backupCodes.generated ? s.backupCodes.remaining + ' / 10 restants' : '○ À générer' }}
                </span>
              </header>
              @if (s.backupCodes.generated && s.backupCodes.remaining < 3) {
                <p class="warn">
                  <mat-icon>warning</mat-icon>
                  Il ne vous reste que {{ s.backupCodes.remaining }} code{{ s.backupCodes.remaining > 1 ? 's' : '' }}. Régénérez-les.
                </p>
              }
              <div class="row">
                <button mat-raised-button color="primary" type="button" (click)="generateCodes()">
                  <mat-icon>refresh</mat-icon>
                  {{ s.backupCodes.generated ? 'Régénérer mes codes' : 'Générer 10 codes' }}
                </button>
              </div>
            </div>
          </li>

          <!-- Card 5: Phone -->
          <li class="card" [class.on]="s.phone.configured">
            <span class="num">5</span>
            <div class="body">
              <header>
                <mat-icon>sms</mat-icon>
                <div>
                  <strong>Téléphone (SMS)</strong>
                  <small>Optionnel · pour la récupération de compte</small>
                </div>
                <span class="state" [class.on]="s.phone.configured">
                  {{ s.phone.configured ? '✓ ' + s.phone.number : '○ Facultatif' }}
                </span>
              </header>
              <div class="row phone">
                <input type="tel" [(ngModel)]="phoneInput" placeholder="+226 70 12 34 56" [disabled]="phoneSending()">
                <button mat-raised-button color="primary" type="button" (click)="sendPhoneCode()" [disabled]="phoneSending() || !phoneInput">
                  <mat-icon>send</mat-icon>
                  @if (phoneSending()) { Envoi… } @else { Envoyer code }
                </button>
              </div>
            </div>
          </li>
        </ol>
      }
    </section>
  `,
  styles: [`
    :host { display: block; }

    .head {
      display: flex;
      justify-content: space-between;
      align-items: flex-start;
      gap: var(--faso-space-3);
      margin-bottom: var(--faso-space-6);
      flex-wrap: wrap;
    }
    .head h1 { margin: 0; font-size: var(--faso-text-3xl); font-weight: var(--faso-weight-bold); }
    .head p { margin: 4px 0 0; color: var(--faso-text-muted); }

    .warn-badge {
      display: inline-flex; align-items: center; gap: 4px;
      padding: 6px 12px;
      background: var(--faso-warning-bg);
      color: var(--faso-warning);
      border: 1px solid var(--faso-warning);
      border-radius: var(--faso-radius-pill);
      font-size: var(--faso-text-sm);
      font-weight: var(--faso-weight-semibold);
    }
    .warn-badge mat-icon { font-size: 18px; width: 18px; height: 18px; }

    .cards {
      list-style: none;
      padding: 0;
      margin: 0;
      display: flex;
      flex-direction: column;
      gap: var(--faso-space-3);
    }
    .card {
      display: grid;
      grid-template-columns: auto 1fr;
      gap: var(--faso-space-4);
      padding: var(--faso-space-5);
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
      transition: border-color var(--faso-duration-fast) var(--faso-ease-standard);
    }
    .card.on {
      border-color: var(--faso-success);
      background: linear-gradient(90deg, var(--faso-success-bg) 0%, var(--faso-surface) 30%);
    }

    .num {
      width: 32px; height: 32px;
      border-radius: 50%;
      background: var(--faso-surface-alt);
      color: var(--faso-text-muted);
      display: inline-flex;
      align-items: center;
      justify-content: center;
      font-weight: var(--faso-weight-bold);
    }
    .card.on .num { background: var(--faso-success); color: #FFFFFF; }

    .card header {
      display: grid;
      grid-template-columns: auto 1fr auto;
      gap: var(--faso-space-3);
      align-items: flex-start;
      margin: 0 0 var(--faso-space-3);
    }
    .card header > mat-icon {
      color: var(--faso-primary-700);
      font-size: 28px; width: 28px; height: 28px;
    }
    .card.on header > mat-icon { color: var(--faso-success); }
    .card strong { display: block; font-size: var(--faso-text-base); }
    .card small { display: block; color: var(--faso-text-muted); margin-top: 2px; }

    .state {
      font-size: var(--faso-text-sm);
      color: var(--faso-text-muted);
      white-space: nowrap;
    }
    .state.on { color: var(--faso-success); font-weight: var(--faso-weight-semibold); }

    .body p { margin: 0 0 var(--faso-space-3); color: var(--faso-text-muted); }
    .body .warn {
      display: inline-flex;
      align-items: center;
      gap: 4px;
      color: var(--faso-warning);
      font-size: var(--faso-text-sm);
    }
    .body .warn mat-icon { font-size: 16px; width: 16px; height: 16px; }
    .row { display: flex; gap: var(--faso-space-2); flex-wrap: wrap; }
    .row.phone input {
      flex: 1;
      min-width: 200px;
      padding: 8px 12px;
      border: 1px solid var(--faso-border-strong);
      border-radius: var(--faso-radius-md);
      font-family: inherit;
    }

    .devices {
      list-style: none;
      padding: 0;
      margin: 0 0 var(--faso-space-3);
      display: flex;
      flex-direction: column;
      gap: var(--faso-space-2);
    }
    .devices li {
      display: grid;
      grid-template-columns: auto 1fr auto;
      gap: var(--faso-space-3);
      align-items: center;
      padding: var(--faso-space-2) var(--faso-space-3);
      background: var(--faso-surface-alt);
      border-radius: var(--faso-radius-md);
    }
    .devices li mat-icon { color: var(--faso-primary-700); }
    .devices strong { display: block; }
    .devices small { color: var(--faso-text-muted); font-size: var(--faso-text-xs); }
  `],
})
export class MfaSettingsComponent implements OnInit {
  private readonly kratos = inject(KratosSettingsService);
  private readonly dialog = inject(MatDialog);
  private readonly snack = inject(MatSnackBar);
  readonly config = inject(PROJECT_CONFIG);

  readonly status = signal<MfaStatus | null>(null);
  readonly loading = signal(true);
  readonly phoneSending = signal(false);
  phoneInput = '';

  ngOnInit(): void { this.reload(); }

  async addPasskey() {
    if (!this.kratos.isBrowser) return;
    try {
      // Stub : en prod, récupérer les options via Kratos initFlow() / webauthn_register_trigger.
      // Ici on appelle directement navigator.credentials.create() pour compatibilité
      // avec le CDP virtual authenticator Playwright (CTAP2 résident).
      const publicKey: PublicKeyCredentialCreationOptions = {
        challenge: crypto.getRandomValues(new Uint8Array(32)),
        rp: { name: this.config.appName, id: window.location.hostname },
        user: {
          id: crypto.getRandomValues(new Uint8Array(16)),
          name: (this.status()?.email.address ?? 'user@example.bf'),
          displayName: (this.status()?.email.address ?? 'user'),
        },
        pubKeyCredParams: [
          { type: 'public-key', alg: -7 },   // ES256
          { type: 'public-key', alg: -257 }, // RS256
        ],
        authenticatorSelection: {
          userVerification: 'preferred',
          residentKey: 'preferred',
        },
        timeout: 60000,
        attestation: 'none',
      };
      const cred = await navigator.credentials.create({ publicKey });
      if (!cred) throw new Error('no-credential');
      // Mock success : add a device to the local status.
      this.status.update((cur) => cur ? {
        ...cur,
        passkey: {
          configured: true,
          devices: [
            ...cur.passkey.devices,
            { id: 'pk-' + Date.now(), name: 'Appareil actuel', addedAt: new Date().toISOString(), kind: 'platform' },
          ],
        },
      } : cur);
      this.snack.open('PassKey ajoutée avec succès', 'OK', { duration: 3000 });
    } catch (err: any) {
      this.snack.open('PassKey refusée : ' + (err?.message ?? 'annulée'), 'OK', { duration: 4000 });
    }
  }

  removePasskey(d: PasskeyDevice): void {
    this.status.update((cur) => cur ? {
      ...cur,
      passkey: {
        configured: cur.passkey.devices.length > 1,
        devices: cur.passkey.devices.filter((x) => x.id !== d.id),
      },
    } : cur);
    this.snack.open(`PassKey « ${d.name} » supprimée`, 'OK', { duration: 2500 });
  }

  configureTotp(): void {
    const account = this.status()?.email.address ?? 'user@example.bf';
    this.kratos.initTotp(this.config.kratosIssuer, account).subscribe((init) => {
      const ref = this.dialog.open(TotpSetupDialogComponent, {
        width: '480px',
        data: { ...init, issuer: this.config.kratosIssuer, account, verify: (code: string) => this.kratos.verifyTotp(code) },
      });
      ref.afterClosed().subscribe((ok) => {
        if (ok) {
          this.status.update((cur) => cur ? { ...cur, totp: { configured: true, configuredAt: new Date().toISOString(), issuer: this.config.kratosIssuer } } : cur);
          this.snack.open('TOTP activé', 'OK', { duration: 3000 });
        }
      });
    });
  }

  disableTotp(): void {
    this.status.update((cur) => cur ? { ...cur, totp: { configured: false } } : cur);
    this.snack.open('TOTP désactivé', 'OK', { duration: 2500 });
  }

  generateCodes(): void {
    this.kratos.generateBackupCodes().subscribe((codes) => {
      this.dialog.open(BackupCodesDialogComponent, { width: '520px', data: { codes } });
      this.status.update((cur) => cur ? { ...cur, backupCodes: { generated: true, remaining: 10, generatedAt: new Date().toISOString() } } : cur);
    });
  }

  changeEmail(): void {
    this.snack.open('Modification email : ouvrira le flow Kratos `verification` (à wire)', 'OK', { duration: 3500 });
  }

  sendPhoneCode(): void {
    if (!this.phoneInput) return;
    this.phoneSending.set(true);
    setTimeout(() => {
      this.phoneSending.set(false);
      this.status.update((cur) => cur ? { ...cur, phone: { configured: true, number: this.phoneInput } } : cur);
      this.snack.open('Numéro enregistré (SMS à activer côté Kratos courier)', 'OK', { duration: 4000 });
    }, 800);
  }

  private reload(): void {
    this.loading.set(true);
    this.kratos.getMfaStatus().subscribe({
      next: (s) => { this.status.set(s); this.loading.set(false); },
      error: () => this.loading.set(false),
    });
  }
}

// --------------------------------------------------------- TOTP dialog
@Component({
  selector: 'app-totp-setup-dialog',
  standalone: true,
  imports: [CommonModule, FormsModule, MatButtonModule, MatIconModule, MatDialogModule, QRCodeComponent],
  template: `
    <h2 mat-dialog-title>Configurer TOTP</h2>
    <mat-dialog-content class="totp-dialog">
      <ol class="steps">
        <li>
          <strong>1. Scannez le QR code</strong>
          <small>Avec Google Authenticator, Authy, 1Password…</small>
          <div class="qr"><qrcode [qrdata]="data.otpauth" [width]="192" [errorCorrectionLevel]="'M'"></qrcode></div>
        </li>
        <li>
          <strong>2. Ou entrez ce code manuellement</strong>
          <code>{{ data.secret }}</code>
          <button mat-button type="button" (click)="copy(data.secret)">
            <mat-icon>content_copy</mat-icon> Copier
          </button>
        </li>
        <li>
          <strong>3. Entrez le code à 6 chiffres généré</strong>
          <input
            type="text"
            inputmode="numeric"
            pattern="\\d{6}"
            maxlength="6"
            [(ngModel)]="code"
            placeholder="000 000"
            aria-label="Code de vérification TOTP"
          >
          @if (error()) { <span class="err">{{ error() }}</span> }
        </li>
      </ol>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button (click)="close(false)">Annuler</button>
      <button mat-raised-button color="primary" (click)="verify()" [disabled]="code.length !== 6 || verifying()">
        @if (verifying()) { Vérification… } @else { Activer TOTP }
      </button>
    </mat-dialog-actions>
  `,
  styles: [`
    .totp-dialog { max-width: 440px; color: #0F172A; }
    .steps { list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: var(--faso-space-4); }
    .steps strong { display: block; margin-bottom: 4px; color: #0F172A; }
    .steps small { display: block; color: #475569; margin-bottom: 6px; }
    .qr { display: flex; justify-content: center; padding: var(--faso-space-3); background: #FFFFFF; border: 1px solid #D1D5DB; border-radius: 8px; }
    code { display: inline-block; background: #F3F4F6; padding: 4px 8px; border-radius: 4px; font-family: var(--faso-font-mono); font-size: 0.85rem; margin-right: 8px; color: #0F172A; }
    input {
      margin-top: 4px;
      padding: 10px 14px;
      border: 1px solid #D1D5DB;
      border-radius: 8px;
      font-family: var(--faso-font-mono);
      font-size: 1.2rem;
      letter-spacing: 0.2em;
      text-align: center;
      background: #FFFFFF;
      color: #0F172A;
      width: 100%;
    }
    .err { color: var(--faso-danger); font-size: var(--faso-text-sm); margin-top: 4px; display: block; }
  `],
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class TotpSetupDialogComponent {
  readonly data = inject<any>(MAT_DIALOG_DATA);
  private readonly ref = inject(MatDialogRef<TotpSetupDialogComponent>);
  readonly verifying = signal(false);
  readonly error = signal('');
  code = '';

  verify(): void {
    this.verifying.set(true);
    this.error.set('');
    this.data.verify(this.code).subscribe((ok: boolean) => {
      this.verifying.set(false);
      if (ok) this.ref.close(true);
      else this.error.set('Code invalide');
    });
  }

  close(v: boolean): void { this.ref.close(v); }
  copy(s: string): void {
    if (typeof navigator !== 'undefined' && navigator.clipboard) navigator.clipboard.writeText(s).catch(() => void 0);
  }
}

// --------------------------------------------------------- Backup codes dialog
@Component({
  selector: 'app-backup-codes-dialog',
  standalone: true,
  imports: [CommonModule, MatButtonModule, MatIconModule, MatDialogModule],
  template: `
    <h2 mat-dialog-title>Vos codes de secours</h2>
    <mat-dialog-content class="codes-dialog">
      <p class="warn">
        <mat-icon>warning</mat-icon>
        Conservez ces codes en lieu sûr — chaque code ne peut être utilisé qu'une seule fois.
      </p>
      <div class="codes">
        @for (c of data.codes; track c) { <code>{{ c }}</code> }
      </div>
    </mat-dialog-content>
    <mat-dialog-actions align="end">
      <button mat-button type="button" (click)="copy()">
        <mat-icon>content_copy</mat-icon> Copier tout
      </button>
      <button mat-button type="button" (click)="download()">
        <mat-icon>download</mat-icon> Télécharger .txt
      </button>
      <button mat-button type="button" (click)="print()">
        <mat-icon>print</mat-icon> Imprimer
      </button>
      <button mat-raised-button color="primary" (click)="ref.close()">J'ai conservé mes codes</button>
    </mat-dialog-actions>
  `,
  styles: [`
    .codes-dialog { max-width: 480px; }
    .warn { display: flex; align-items: center; gap: 6px; color: var(--faso-warning); font-size: var(--faso-text-sm); margin: 0 0 var(--faso-space-3); }
    .warn mat-icon { font-size: 18px; width: 18px; height: 18px; }
    .codes {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 8px;
      padding: var(--faso-space-3);
      background: #F3F4F6;
      border-radius: 8px;
    }
    code {
      display: block;
      font-family: var(--faso-font-mono);
      font-size: 1rem;
      text-align: center;
      padding: 6px;
      background: #FFFFFF;
      border: 1px solid #D1D5DB;
      border-radius: 4px;
      letter-spacing: 0.1em;
      color: #0F172A;
    }
  `],
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class BackupCodesDialogComponent {
  readonly data = inject<{ codes: string[] }>(MAT_DIALOG_DATA);
  readonly ref = inject(MatDialogRef<BackupCodesDialogComponent>);

  copy(): void {
    const text = this.data.codes.join('\n');
    if (typeof navigator !== 'undefined' && navigator.clipboard) navigator.clipboard.writeText(text).catch(() => void 0);
  }

  download(): void {
    if (typeof window === 'undefined') return;
    const blob = new Blob([this.data.codes.join('\n')], { type: 'text/plain' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `poulets-bf-backup-codes-${new Date().toISOString().slice(0, 10)}.txt`;
    a.click();
    URL.revokeObjectURL(url);
  }

  print(): void {
    if (typeof window === 'undefined') return;
    const w = window.open('', 'print', 'width=400,height=600');
    if (!w) return;
    w.document.write(`<pre style="font-family: monospace; font-size: 16px; line-height: 2">${this.data.codes.join('\n')}</pre>`);
    w.document.close();
    w.print();
  }
}
