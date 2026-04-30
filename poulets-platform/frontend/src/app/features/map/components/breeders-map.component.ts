// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import {
  ChangeDetectionStrategy, Component, ElementRef, OnDestroy, OnInit, PLATFORM_ID, ViewChild, inject, signal,
} from '@angular/core';
import { CommonModule, isPlatformBrowser } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';

import { BreederProfileService } from '@features/profile/services/breeder-profile.service';
import { BreederProfile } from '@shared/models/reputation.models';

@Component({
  selector: 'app-breeders-map',
  standalone: true,
  imports: [CommonModule, RouterLink, MatIconModule, MatButtonModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="wrap" data-testid="map-page">
      <header class="head">
        <div>
          <h1>Carte des éleveurs</h1>
          <p data-testid="map-detail-field-count">{{ count() }} éleveurs {{ count() > 1 ? 'vérifiés' : 'vérifié' }} au Burkina Faso</p>
        </div>
        <a mat-stroked-button routerLink="/marketplace/annonces" data-testid="map-action-list-view">
          <mat-icon>list</mat-icon>
          Voir la liste
        </a>
      </header>

      <div class="map-shell" data-testid="map-container">
        <div #mapHost id="breeders-map" class="map" data-testid="map-canvas"></div>
        @if (!ready()) {
          <div class="overlay" data-testid="map-loading">
            <mat-icon>map</mat-icon>
            <p>@if (ssr) { Carte indisponible côté serveur } @else { Chargement de la carte… }</p>
          </div>
        }
      </div>
    </div>
  `,
  styles: [`
    :host { display: block; height: 100%; }
    .wrap {
      max-width: 1400px;
      margin: 0 auto;
      padding: var(--faso-space-6) var(--faso-space-4) var(--faso-space-10);
    }
    .head {
      display: flex;
      justify-content: space-between;
      align-items: flex-end;
      gap: var(--faso-space-3);
      margin-bottom: var(--faso-space-5);
      flex-wrap: wrap;
    }
    h1 {
      margin: 0;
      font-size: var(--faso-text-3xl);
      font-weight: var(--faso-weight-bold);
    }
    h1 + p { margin: 4px 0 0; color: var(--faso-text-muted); }

    .map-shell {
      position: relative;
      height: clamp(440px, 70vh, 720px);
      border-radius: var(--faso-radius-xl);
      overflow: hidden;
      border: 1px solid var(--faso-border);
      box-shadow: var(--faso-shadow-md);
    }
    .map { width: 100%; height: 100%; }

    .overlay {
      position: absolute;
      inset: 0;
      display: flex;
      flex-direction: column;
      align-items: center;
      justify-content: center;
      gap: var(--faso-space-2);
      background: var(--faso-surface-alt);
      color: var(--faso-text-muted);
    }
    .overlay mat-icon { font-size: 48px; width: 48px; height: 48px; color: var(--faso-primary-400); }

    :global(.breeder-popup) {
      min-width: 220px;
    }
    :global(.breeder-popup h3) {
      margin: 0 0 4px;
      font-size: 1rem;
      font-weight: 600;
    }
    :global(.breeder-popup .meta) {
      color: #64748B;
      font-size: 0.85rem;
      margin: 0 0 8px;
    }
    :global(.breeder-popup a) {
      color: #1B5E20;
      font-weight: 600;
      font-size: 0.85rem;
    }
  `],
})
export class BreedersMapComponent implements OnInit, OnDestroy {
  @ViewChild('mapHost', { static: true }) host!: ElementRef<HTMLDivElement>;

  private readonly platformId = inject(PLATFORM_ID);
  private readonly svc = inject(BreederProfileService);

  readonly ready = signal(false);
  readonly count = signal(0);
  readonly ssr = !isPlatformBrowser(this.platformId);

  private map: any = null;
  private markers: any[] = [];

  async ngOnInit(): Promise<void> {
    if (!isPlatformBrowser(this.platformId)) return;

    // Load Leaflet CSS once, on-demand, when the map page is visited.
    if (!document.getElementById('leaflet-css')) {
      const link = document.createElement('link');
      link.id = 'leaflet-css';
      link.rel = 'stylesheet';
      link.href = 'https://unpkg.com/leaflet@1.9.4/dist/leaflet.css';
      document.head.appendChild(link);
    }

    const L = await import('leaflet');
    // Leaflet icon images are referenced by URL — fix default icon for bundler.
    const iconRetinaUrl = 'https://unpkg.com/leaflet@1.9.4/dist/images/marker-icon-2x.png';
    const iconUrl = 'https://unpkg.com/leaflet@1.9.4/dist/images/marker-icon.png';
    const shadowUrl = 'https://unpkg.com/leaflet@1.9.4/dist/images/marker-shadow.png';
    const DefaultIcon = L.icon({
      iconRetinaUrl, iconUrl, shadowUrl,
      iconSize: [25, 41], iconAnchor: [12, 41], popupAnchor: [1, -34],
      tooltipAnchor: [16, -28], shadowSize: [41, 41],
    });
    (L.Marker.prototype as any).options.icon = DefaultIcon;

    this.map = L.map(this.host.nativeElement, {
      center: [12.37, -1.52],
      zoom: 7,
      scrollWheelZoom: true,
    });

    L.tileLayer('https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', {
      attribution: '&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a>',
      maxZoom: 18,
    }).addTo(this.map);

    this.svc.list().subscribe((list) => {
      this.addMarkers(L, list);
      this.count.set(list.length);
    });

    this.ready.set(true);
  }

  ngOnDestroy(): void {
    if (this.map) { this.map.remove(); this.map = null; }
  }

  private addMarkers(L: any, breeders: BreederProfile[]) {
    const bounds: [number, number][] = [];
    for (const b of breeders) {
      if (b.latitude == null || b.longitude == null) continue;
      const marker = L.marker([b.latitude, b.longitude]);
      const full = b.prenom ? `${b.prenom} ${b.name}` : b.name;
      const html = `
        <div class="breeder-popup">
          <h3>${full}</h3>
          <p class="meta">${b.city ?? ''}, ${b.region}</p>
          <p class="meta">${b.specialties.slice(0, 3).join(' · ')}</p>
          <a href="/profile/eleveur/${b.id}">Voir le profil →</a>
        </div>
      `;
      marker.bindPopup(html);
      marker.addTo(this.map);
      this.markers.push(marker);
      bounds.push([b.latitude, b.longitude]);
    }
    if (bounds.length > 1) {
      this.map.fitBounds(bounds, { padding: [40, 40] });
    }
  }
}
