// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component } from '@angular/core';
import { CommonModule } from '@angular/common';

// TODO(FASO-F6): intégrer service KYC (Onfido/Veriff) + validation selfie biométrique
//   - Brancher SDK KYC (Onfido Web SDK ou équivalent souverain à évaluer)
//   - Endpoint backend /api/profile/kyc/verify (auth-ms) pour POST des pièces
//   - Validation CNIB : OCR + check-digit + déduplication (KAYA set de hashs)
//   - Selfie : liveness detection + comparaison embedding visage
//   - Statut : PENDING / IN_REVIEW / APPROVED / REJECTED
//   - Logs auditables dans Postgres (table kyc_verification_audit)

@Component({
  selector: 'app-kyc',
  standalone: true,
  imports: [CommonModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="stub">
      <h1>Vérification d'identité</h1>
      <p>TODO(FASO-F6): intégrer service KYC (Onfido/Veriff) + validation selfie biométrique</p>

      <form class="form" aria-label="Formulaire KYC (stub désactivé)">
        <label class="field">
          <span>Pièce d'identité (CNIB)</span>
          <input type="file" accept="image/*,.pdf" [disabled]="true" data-testid="kyc-cnib" />
        </label>

        <label class="field">
          <span>Selfie de vérification</span>
          <input type="file" accept="image/*" [disabled]="true" data-testid="kyc-selfie" />
        </label>

        <button type="button" [disabled]="true">Soumettre (indisponible)</button>
      </form>
    </section>
  `,
  styles: [`
    .stub { padding: 24px; max-width: 720px; margin: 0 auto; }
    .stub h1 { font-size: 1.75rem; margin-bottom: 12px; }
    .stub p { color: #555; margin: 8px 0 24px; }
    .form { display: flex; flex-direction: column; gap: 16px; }
    .field { display: flex; flex-direction: column; gap: 6px; }
    .field span { font-weight: 500; color: #333; }
    .field input[type=file] { padding: 8px; border: 1px solid #ccc; border-radius: 4px; background: #fafafa; }
    button { padding: 10px 16px; border-radius: 4px; border: none; background: #ccc; color: #666; cursor: not-allowed; }
  `],
})
export class KycComponent {}
