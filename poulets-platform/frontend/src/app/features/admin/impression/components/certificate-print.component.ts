// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso
//
// NOTE: La génération PDF côté client a été abandonnée. Les certificats
// seront rendus par le microservice backend `ec-certificate-renderer`
// (aligné avec le pattern ETAT-CIVIL / SOGESY).
//
// Ce fichier reste comme stub pour l'intégration future : ajouter un
// wrapper HTTP qui GET /api/certificate-renderer/{type}/{id}.pdf et
// affiche le résultat dans un <iframe> ou propose un download direct.

import { ChangeDetectionStrategy, Component } from '@angular/core';

@Component({
  selector: 'app-certificate-print',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="stub">
      <h1>Service en cours d'intégration</h1>
      <p>Le rendu PDF sera délégué au microservice <code>ec-certificate-renderer</code>.</p>
    </section>
  `,
  styles: [`
    .stub {
      max-width: 520px;
      margin: 10vh auto;
      padding: 2rem;
      text-align: center;
      color: var(--faso-text-muted);
    }
    h1 { color: var(--faso-text); margin: 0 0 0.5rem; }
    code { font-family: var(--faso-font-mono); background: var(--faso-surface-alt); padding: 2px 6px; border-radius: 4px; }
  `],
})
export class CertificatePrintComponent {}
