// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component } from '@angular/core';
import { CommonModule } from '@angular/common';

// TODO(FASO-F5): activer service worker, cache strategy, offline fallback
//   - NE PAS exécuter `ng add @angular/pwa` ici (risque de casser le build)
//   - Approche recommandée : ajouter manifest + service-worker.js dans src/
//     avec stratégie StaleWhileRevalidate sur GET /api/marketplace/*
//     et CacheFirst sur les assets statiques
//   - Ajouter fallback offline page (cette page) via catch handler
//   - Tester avec Chrome DevTools → Application → Offline

@Component({
  selector: 'app-pwa-info',
  standalone: true,
  imports: [CommonModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="stub">
      <h1>Mode hors-ligne</h1>
      <p>TODO(FASO-F5): activer service worker, cache strategy, offline fallback</p>
      <div class="placeholder" aria-label="Indicateur offline (à venir)">
        <em>Statut PWA : non installé (stub MVP)</em>
      </div>
    </section>
  `,
  styles: [`
    .stub { padding: 24px; max-width: 720px; margin: 0 auto; }
    .stub h1 { font-size: 1.75rem; margin-bottom: 12px; }
    .stub p { color: #555; margin: 8px 0; }
    .stub .placeholder {
      margin-top: 24px;
      min-height: 160px;
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
export class PwaInfoComponent {}
