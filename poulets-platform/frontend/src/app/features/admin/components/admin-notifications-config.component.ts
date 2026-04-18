// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';
import { MatSnackBar } from '@angular/material/snack-bar';
import { SectionHeaderComponent } from '@shared/components/section-header/section-header.component';

interface Channel  { key: 'email' | 'sms' | 'push' | 'webhook'; label: string; icon: string; enabled: boolean; }
interface Template { key: string; label: string; subject: string; body: string; }

@Component({
  selector: 'app-admin-notifications-config',
  standalone: true,
  imports: [CommonModule, FormsModule, MatIconModule, MatButtonModule, SectionHeaderComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="page">
      <header>
        <h1>Notifications — Configuration</h1>
        <p>Canaux, modèles d'emails, serveur SMTP</p>
      </header>

      <app-section-header title="Canaux" kicker="Activer / désactiver" />
      <div class="channels">
        @for (c of channels(); track c.key) {
          <article class="channel" [class.on]="c.enabled">
            <mat-icon>{{ c.icon }}</mat-icon>
            <div>
              <strong>{{ c.label }}</strong>
              <small>{{ c.enabled ? 'Actif' : 'Inactif' }}</small>
            </div>
            <label class="switch">
              <input type="checkbox" [checked]="c.enabled" (change)="toggle(c.key, $event)">
              <span class="track"><span class="thumb"></span></span>
            </label>
          </article>
        }
      </div>

      <app-section-header title="Serveur SMTP" kicker="Envoi d'emails" />
      <div class="smtp">
        <label class="field">
          <span>Host</span>
          <input [(ngModel)]="smtp.host" type="text" placeholder="smtp.gmail.com">
        </label>
        <label class="field">
          <span>Port</span>
          <input [(ngModel)]="smtp.port" type="number" placeholder="465">
        </label>
        <label class="field">
          <span>From (email)</span>
          <input [(ngModel)]="smtp.from" type="email" placeholder="fasodigitalisation@gmail.com">
        </label>
        <label class="field">
          <span>From (nom)</span>
          <input [(ngModel)]="smtp.fromName" type="text" placeholder="Poulets BF">
        </label>
        <label class="field full">
          <span>Mot de passe d'application (référence Vault)</span>
          <input [(ngModel)]="smtp.pwdRef" type="text" placeholder="secret/smtp/gmail_app_password" readonly>
        </label>
        <div class="actions">
          <button mat-raised-button color="primary" type="button" (click)="testSmtp()">
            <mat-icon>outgoing_mail</mat-icon> Envoyer un email test
          </button>
          <button mat-stroked-button type="button" (click)="saveSmtp()">
            <mat-icon>save</mat-icon> Enregistrer
          </button>
        </div>
      </div>

      <app-section-header title="Modèles d'emails" kicker="Personnaliser contenu et ton" />
      <div class="templates">
        @for (t of templates(); track t.key) {
          <article class="tmpl">
            <header>
              <div>
                <code>{{ t.key }}</code>
                <strong>{{ t.label }}</strong>
              </div>
              <button mat-button type="button" (click)="preview(t)">
                <mat-icon>visibility</mat-icon> Aperçu
              </button>
            </header>
            <label class="field">
              <span>Sujet</span>
              <input [(ngModel)]="t.subject" type="text">
            </label>
            <label class="field">
              <span>Corps (HTML/MJML, variables : @{{ '{{' }}name@{{ '}}' }} , @{{ '{{' }}orderId@{{ '}}' }})</span>
              <textarea [(ngModel)]="t.body" rows="4"></textarea>
            </label>
          </article>
        }
      </div>
    </section>
  `,
  styles: [`
    :host { display: block; }
    header { margin-bottom: var(--faso-space-6); }
    header h1 { margin: 0; font-size: var(--faso-text-3xl); font-weight: var(--faso-weight-bold); }
    header p { margin: 4px 0 0; color: var(--faso-text-muted); }

    .channels {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
      gap: var(--faso-space-3);
      margin-bottom: var(--faso-space-8);
    }
    .channel {
      display: grid;
      grid-template-columns: auto 1fr auto;
      gap: var(--faso-space-3);
      align-items: center;
      padding: var(--faso-space-4);
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
    }
    .channel.on { border-color: var(--faso-success); background: var(--faso-success-bg); }
    .channel mat-icon {
      width: 40px; height: 40px;
      border-radius: 10px;
      background: var(--faso-primary-50);
      color: var(--faso-primary-700);
      display: inline-flex;
      align-items: center;
      justify-content: center;
    }
    .channel strong { display: block; }
    .channel small { color: var(--faso-text-muted); font-size: var(--faso-text-xs); }

    .switch { position: relative; cursor: pointer; }
    .switch input { position: absolute; opacity: 0; }
    .switch .track {
      display: inline-block;
      width: 44px; height: 24px;
      background: var(--faso-border-strong);
      border-radius: var(--faso-radius-pill);
      position: relative;
      transition: background var(--faso-duration-fast) var(--faso-ease-standard);
    }
    .switch .thumb {
      position: absolute; top: 2px; left: 2px;
      width: 20px; height: 20px;
      background: #FFFFFF;
      border-radius: 50%;
      transition: left var(--faso-duration-fast) var(--faso-ease-standard);
    }
    .switch input:checked + .track { background: var(--faso-primary-600); }
    .switch input:checked + .track .thumb { left: 22px; }

    .smtp {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: var(--faso-space-3);
      padding: var(--faso-space-5);
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
      margin-bottom: var(--faso-space-8);
    }
    .smtp .full { grid-column: 1 / -1; }
    .field { display: flex; flex-direction: column; gap: 4px; }
    .field span {
      font-size: var(--faso-text-xs);
      font-weight: var(--faso-weight-semibold);
      color: var(--faso-text-muted);
      text-transform: uppercase;
      letter-spacing: 0.04em;
    }
    .field input, .field textarea {
      padding: 8px 12px;
      border: 1px solid var(--faso-border-strong);
      border-radius: var(--faso-radius-md);
      font-family: inherit;
      font-size: var(--faso-text-sm);
      background: var(--faso-surface);
      color: var(--faso-text);
      resize: vertical;
    }
    .field input:focus, .field textarea:focus {
      outline: none;
      border-color: var(--faso-primary-500);
      box-shadow: 0 0 0 3px var(--faso-primary-100);
    }
    .smtp .actions { grid-column: 1 / -1; display: flex; gap: var(--faso-space-2); flex-wrap: wrap; }

    .templates { display: flex; flex-direction: column; gap: var(--faso-space-3); }
    .tmpl {
      padding: var(--faso-space-4);
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
      display: grid;
      grid-template-columns: 1fr;
      gap: var(--faso-space-3);
    }
    .tmpl header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin: 0;
    }
    .tmpl code {
      background: var(--faso-surface-alt);
      padding: 2px 6px;
      border-radius: var(--faso-radius-sm);
      font-family: var(--faso-font-mono);
      font-size: var(--faso-text-xs);
      margin-right: 8px;
    }
    .tmpl strong { font-size: var(--faso-text-base); }

    @media (max-width: 639px) {
      .smtp { grid-template-columns: 1fr; }
    }
  `],
})
export class AdminNotificationsConfigComponent {
  private readonly snack = inject(MatSnackBar);

  readonly channels = signal<Channel[]>([
    { key: 'email',   label: 'Email',      icon: 'mail',            enabled: true },
    { key: 'sms',     label: 'SMS',        icon: 'sms',             enabled: false },
    { key: 'push',    label: 'Push navigateur', icon: 'notifications', enabled: false },
    { key: 'webhook', label: 'Webhook',    icon: 'webhook',         enabled: true },
  ]);

  smtp = {
    host: 'smtp.gmail.com',
    port: 465,
    from: 'fasodigitalisation@gmail.com',
    fromName: 'Poulets BF',
    pwdRef: 'secret/smtp/gmail_app_password',
  };

  readonly templates = signal<Template[]>([
    { key: 'order.confirmation', label: 'Commande confirmée',
      subject: 'Votre commande {{orderId}} est confirmée',
      body: 'Bonjour {{name}},\n\nVotre éleveur {{breederName}} a confirmé la commande {{orderId}}. Livraison prévue le {{deliveryDate}}.\n\nL\'équipe Poulets BF' },
    { key: 'order.delivered', label: 'Commande livrée',
      subject: '{{orderId}} : livraison confirmée',
      body: 'Merci {{name}} ! Votre commande a été livrée. N\'oubliez pas de laisser un avis à {{breederName}}.' },
    { key: 'mfa.reminder', label: 'Rappel MFA',
      subject: 'Sécurisez votre compte Poulets BF',
      body: 'Bonjour {{name}},\n\nVotre compte n\'a pas encore de méthode d\'authentification à deux facteurs. Ajoutez une PassKey dès maintenant : {{mfaLink}}' },
    { key: 'halal.step_completed', label: 'Étape halal validée',
      subject: 'Étape halal {{stepNumber}} validée',
      body: 'Bonjour {{name}},\n\nL\'étape "{{stepLabel}}" de votre certification halal pour le lot {{lotId}} vient d\'être validée.' },
    { key: 'review.received', label: 'Nouvel avis',
      subject: 'Nouvel avis {{rating}}★ reçu',
      body: '{{reviewerName}} vient de vous laisser un avis :\n\n"{{reviewText}}"' },
  ]);

  toggle(key: Channel['key'], ev: Event): void {
    const enabled = (ev.target as HTMLInputElement).checked;
    this.channels.update((arr) => arr.map((c) => c.key === key ? { ...c, enabled } : c));
    this.snack.open(`${key} ${enabled ? 'activé' : 'désactivé'}`, 'OK', { duration: 2500 });
  }

  testSmtp(): void { this.snack.open('Email test envoyé (stub)', 'OK', { duration: 2500 }); }
  saveSmtp(): void { this.snack.open('Configuration SMTP enregistrée', 'OK', { duration: 2500 }); }
  preview(t: Template): void {
    this.snack.open(`Aperçu « ${t.key} » (à brancher sur preview modal)`, 'OK', { duration: 2500 });
  }
}
