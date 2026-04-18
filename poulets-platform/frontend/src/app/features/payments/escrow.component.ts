// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { ActivatedRoute } from '@angular/router';

// TODO(FASO-F7): brancher Temporal workflow escrow-transaction
//   - Workflow Temporal : EscrowTransactionWorkflow (Java worker dans poulets-api)
//   - Activities : holdFunds (Orange Money API), releaseFunds, refundFunds
//   - États : HELD → RELEASED (livraison OK) | REFUNDED (dispute) | EXPIRED (timeout 7j)
//   - Query endpoint GraphQL : escrowTx(id) → { state, amount, createdAt, deliveryId }
//   - Subscription GraphQL pour mise à jour temps réel
//   - Signature HMAC côté BFF pour protéger release

type EscrowState = 'HELD' | 'RELEASED' | 'REFUNDED';

@Component({
  selector: 'app-escrow',
  standalone: true,
  imports: [CommonModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="stub">
      <h1>Paiement sécurisé (séquestre)</h1>
      <p>TODO(FASO-F7): brancher Temporal workflow escrow-transaction</p>

      <dl class="meta">
        <dt>Transaction</dt>
        <dd><code>{{ txId() }}</code></dd>
        <dt>État actuel</dt>
        <dd>
          <span class="badge" [class]="'state-' + currentState().toLowerCase()">
            {{ currentState() }}
          </span>
        </dd>
      </dl>

      <ul class="states" aria-label="États possibles du séquestre">
        @for (s of states; track s) {
          <li class="state-item" [class.active]="s === currentState()">
            <span class="dot"></span>
            <span>{{ s }}</span>
          </li>
        }
      </ul>
    </section>
  `,
  styles: [`
    .stub { padding: 24px; max-width: 720px; margin: 0 auto; }
    .stub h1 { font-size: 1.75rem; margin-bottom: 12px; }
    .stub p { color: #555; margin: 8px 0 24px; }
    .meta { display: grid; grid-template-columns: auto 1fr; gap: 8px 16px; margin-bottom: 24px; }
    .meta dt { font-weight: 500; color: #666; }
    .meta dd { margin: 0; }
    .meta code { background: #f3f3f3; padding: 2px 6px; border-radius: 4px; }
    .badge { padding: 4px 10px; border-radius: 12px; font-weight: 500; font-size: 0.85rem; }
    .state-held { background: #fff4cc; color: #7a5a00; }
    .state-released { background: #d4f1d4; color: #2a6a2a; }
    .state-refunded { background: #f7d4d4; color: #7a2a2a; }
    .states { list-style: none; padding: 0; display: flex; gap: 24px; justify-content: space-between; max-width: 480px; }
    .state-item { display: flex; align-items: center; gap: 8px; color: #888; }
    .state-item.active { color: #222; font-weight: 600; }
    .state-item .dot { width: 12px; height: 12px; border-radius: 50%; background: #ccc; }
    .state-item.active .dot { background: #3b82f6; }
  `],
})
export class EscrowComponent {
  private readonly route = inject(ActivatedRoute);

  readonly states: readonly EscrowState[] = ['HELD', 'RELEASED', 'REFUNDED'];
  readonly txId = signal<string>(
    this.route.snapshot.paramMap.get('txId') ?? 'inconnu',
  );
  readonly currentState = signal<EscrowState>('HELD');
}
