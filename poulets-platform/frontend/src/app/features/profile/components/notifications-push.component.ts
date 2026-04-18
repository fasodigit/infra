// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, signal } from '@angular/core';
import { CommonModule } from '@angular/common';

// TODO(FASO-F8): Web Push API + VAPID keys + service worker subscribe
//   - Générer paire VAPID (public/private) côté notifier-ms (Java)
//   - Exposer publicKey via GET /api/notifications/push/vapid-public-key
//   - Côté client : navigator.serviceWorker.ready → pushManager.subscribe({
//       userVisibleOnly: true, applicationServerKey: vapidPublicKey })
//   - POST subscription vers /api/notifications/push/subscribe (BFF)
//   - notifier-ms stocke subscription par user_id (Postgres) et envoie
//     via web-push lib côté Java (jnanthrah/web-push-java)
//   - Fallback Firebase Cloud Messaging refusé : souveraineté FASO

@Component({
  selector: 'app-notifications-push',
  standalone: true,
  imports: [CommonModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="stub">
      <h1>Notifications push</h1>
      <p>TODO(FASO-F8): Web Push API + VAPID keys + service worker subscribe</p>

      <label class="toggle">
        <input
          type="checkbox"
          [checked]="enabled()"
          [disabled]="true"
          data-testid="push-toggle"
        />
        <span>Activer notifications push</span>
      </label>
      <p class="hint">Fonctionnalité en cours de préparation (stub MVP).</p>
    </section>
  `,
  styles: [`
    .stub { padding: 24px; max-width: 720px; margin: 0 auto; }
    .stub h1 { font-size: 1.75rem; margin-bottom: 12px; }
    .stub p { color: #555; margin: 8px 0; }
    .toggle { display: flex; align-items: center; gap: 12px; padding: 16px; border: 1px solid #e0e0e0; border-radius: 6px; background: #fafafa; }
    .toggle input[type=checkbox] { width: 20px; height: 20px; }
    .hint { font-size: 0.85rem; color: #888; font-style: italic; }
  `],
})
export class NotificationsPushComponent {
  readonly enabled = signal<boolean>(false);
}
