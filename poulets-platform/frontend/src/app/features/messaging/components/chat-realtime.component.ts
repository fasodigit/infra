// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, inject, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { ActivatedRoute } from '@angular/router';

// TODO(FASO-F4): intégrer socket.io-client + endpoint /ws/chat
//   - Ajouter dépendance socket.io-client dans package.json
//   - Créer ChatRealtimeService avec WebSocket wrapper
//   - Brancher sur BFF endpoint /ws/chat (réservé port 4800)
//   - Gérer reconnect/backoff, typing indicator, read receipts
//   - Persister messages offline dans IndexedDB

@Component({
  selector: 'app-chat-realtime',
  standalone: true,
  imports: [CommonModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="stub">
      <h1>Chat temps réel</h1>
      <p>TODO(FASO-F4): intégrer socket.io-client + endpoint /ws/chat</p>
      <p class="meta">Conversation : <code>{{ conversationId() }}</code></p>
      <div class="placeholder" aria-label="Zone de chat (à venir)">
        <em>Zone messagerie temps réel (stub MVP)</em>
      </div>
    </section>
  `,
  styles: [`
    .stub { padding: 24px; max-width: 720px; margin: 0 auto; }
    .stub h1 { font-size: 1.75rem; margin-bottom: 12px; }
    .stub p { color: #555; margin: 8px 0; }
    .stub .meta code { background: #f3f3f3; padding: 2px 6px; border-radius: 4px; }
    .stub .placeholder {
      margin-top: 24px;
      min-height: 240px;
      border: 2px dashed #ccc;
      border-radius: 8px;
      display: flex;
      align-items: center;
      justify-content: center;
      color: #888;
      font-style: italic;
    }
  `],
})
export class ChatRealtimeComponent {
  private readonly route = inject(ActivatedRoute);

  readonly conversationId = signal<string>(
    this.route.snapshot.paramMap.get('conversationId') ?? 'inconnu',
  );
}
