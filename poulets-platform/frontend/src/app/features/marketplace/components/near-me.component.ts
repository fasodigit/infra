// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, signal } from '@angular/core';
import { CommonModule } from '@angular/common';

// TODO(FASO-F9): Leaflet + OpenStreetMap + GeoJSON communes BF
//   - Dépendance Leaflet déjà présente (package.json)
//   - Charger GeoJSON des 351 communes BF depuis /assets/geo/communes-bf.json
//     (source : IGB Burkina Faso / OpenStreetMap Overpass export)
//   - Tuiles OSM via tile.openstreetmap.org (respecter usage policy) ou
//     déployer tileserver-gl souverain (à évaluer dans INFRA/docker/compose/)
//   - Géoloc utilisateur : navigator.geolocation.getCurrentPosition
//   - Requête GraphQL : nearbyOffers(lat, lng, radiusKm) → poulets-api
//     avec index géospatial KAYA (commande GEOSEARCH / GEODIST)

@Component({
  selector: 'app-near-me',
  standalone: true,
  imports: [CommonModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="stub">
      <h1>À proximité</h1>
      <p>TODO(FASO-F9): Leaflet + OpenStreetMap + GeoJSON communes BF</p>

      <label class="field">
        <span>Rayon de recherche (km)</span>
        <input
          type="number"
          min="1"
          max="100"
          [value]="radiusKm()"
          [disabled]="true"
          data-testid="near-me-radius"
        />
      </label>

      <div class="map-placeholder" aria-label="Carte interactive (à venir)">
        <em>Carte à venir</em>
      </div>
    </section>
  `,
  styles: [`
    .stub { padding: 24px; max-width: 960px; margin: 0 auto; }
    .stub h1 { font-size: 1.75rem; margin-bottom: 12px; }
    .stub p { color: #555; margin: 8px 0 24px; }
    .field { display: flex; flex-direction: column; gap: 6px; margin-bottom: 24px; max-width: 320px; }
    .field span { font-weight: 500; color: #333; }
    .field input { padding: 8px; border: 1px solid #ccc; border-radius: 4px; background: #fafafa; }
    .map-placeholder {
      height: 360px;
      border: 2px dashed #ccc;
      border-radius: 8px;
      display: flex;
      align-items: center;
      justify-content: center;
      color: #888;
      font-style: italic;
      background: linear-gradient(135deg, #f7f7f7 0%, #efefef 100%);
    }
  `],
})
export class NearMeComponent {
  readonly radiusKm = signal<number>(10);
}
