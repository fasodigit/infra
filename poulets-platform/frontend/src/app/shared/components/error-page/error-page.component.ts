// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, Input } from '@angular/core';
import { CommonModule } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';

export type ErrorKind = '404' | '500' | 'offline' | 'forbidden';

interface Preset { icon: string; title: string; desc: string; }

const PRESETS: Record<ErrorKind, Preset> = {
  '404':      { icon: 'travel_explore', title: 'Page introuvable',           desc: 'La page que vous cherchez n\'existe pas ou a été déplacée.' },
  '500':      { icon: 'build_circle',   title: 'Erreur serveur',             desc: 'Une erreur est survenue de notre côté. L\'équipe a été notifiée.' },
  'offline':  { icon: 'wifi_off',       title: 'Vous êtes hors ligne',       desc: 'Vérifiez votre connexion internet puis réessayez.' },
  'forbidden':{ icon: 'lock',           title: 'Accès refusé',               desc: 'Vous n\'avez pas les permissions pour voir cette page.' },
};

@Component({
  selector: 'app-error-page',
  standalone: true,
  imports: [CommonModule, RouterLink, MatIconModule, MatButtonModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="wrap">
      <div class="inner">
        <span class="icon-wrap"><mat-icon>{{ preset.icon }}</mat-icon></span>
        <h1 class="code">{{ code }}</h1>
        <h2>{{ preset.title }}</h2>
        <p>{{ preset.desc }}</p>
        <div class="actions">
          <a mat-raised-button color="primary" routerLink="/">Retour à l'accueil</a>
          @if (kind === 'offline') {
            <button mat-stroked-button type="button" (click)="reload()">Réessayer</button>
          } @else {
            <a mat-stroked-button routerLink="/marketplace/annonces">Explorer le marketplace</a>
          }
        </div>
      </div>
    </section>
  `,
  styles: [`
    :host { display: block; background: var(--faso-bg); min-height: 100vh; }
    .wrap {
      display: flex;
      align-items: center;
      justify-content: center;
      min-height: 80vh;
      padding: var(--faso-space-10) var(--faso-space-4);
    }
    .inner {
      max-width: 520px;
      text-align: center;
      padding: var(--faso-space-8);
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
      box-shadow: var(--faso-shadow-sm);
    }
    .icon-wrap {
      display: inline-flex;
      width: 80px;
      height: 80px;
      border-radius: 50%;
      background: var(--faso-accent-100);
      color: var(--faso-accent-800);
      align-items: center;
      justify-content: center;
      margin-bottom: var(--faso-space-4);
    }
    .icon-wrap mat-icon { font-size: 40px; width: 40px; height: 40px; }
    .code {
      font-size: 4rem;
      font-weight: 800;
      color: var(--faso-primary-700);
      margin: 0;
      line-height: 1;
      letter-spacing: -0.02em;
    }
    h2 {
      margin: var(--faso-space-2) 0 var(--faso-space-2);
      font-size: var(--faso-text-2xl);
      font-weight: var(--faso-weight-semibold);
    }
    p {
      color: var(--faso-text-muted);
      max-width: 44ch;
      margin: 0 auto var(--faso-space-6);
    }
    .actions {
      display: flex;
      flex-wrap: wrap;
      gap: var(--faso-space-2);
      justify-content: center;
    }
  `],
})
export class ErrorPageComponent {
  @Input() kind: ErrorKind = '404';

  get preset(): Preset { return PRESETS[this.kind]; }
  get code(): string {
    switch (this.kind) {
      case '404':       return '404';
      case '500':       return '500';
      case 'forbidden': return '403';
      case 'offline':   return '—';
    }
  }

  reload(): void {
    if (typeof window !== 'undefined') window.location.reload();
  }
}
