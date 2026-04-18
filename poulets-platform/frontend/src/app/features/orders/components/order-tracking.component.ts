// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import {
  ChangeDetectionStrategy, Component, ElementRef, OnDestroy, OnInit,
  PLATFORM_ID, ViewChild, inject, signal,
} from '@angular/core';
import { CommonModule, DatePipe, isPlatformBrowser } from '@angular/common';
import { ActivatedRoute, RouterLink } from '@angular/router';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';

import { StatusBadgeComponent } from '@shared/components/status-badge/status-badge.component';

interface TimelineStep {
  key: string;
  label: string;
  icon: string;
  at?: string;
  done: boolean;
  current?: boolean;
}

@Component({
  selector: 'app-order-tracking',
  standalone: true,
  imports: [CommonModule, RouterLink, DatePipe, MatIconModule, MatButtonModule, StatusBadgeComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="page">
      <div class="container">
        <header>
          <a mat-button routerLink="/orders" class="back">
            <mat-icon>arrow_back</mat-icon> Mes commandes
          </a>
          <div class="head">
            <div>
              <h1>Commande {{ orderNumber() }}</h1>
              <p>Passée le {{ createdAt() | date:'mediumDate' }}</p>
            </div>
            <app-status-badge [status]="currentStatus()" />
          </div>
        </header>

        <div class="grid">
          <article class="card">
            <h2>Suivi de la livraison</h2>
            <ol class="timeline">
              @for (step of timeline(); track step.key) {
                <li [class.done]="step.done" [class.current]="step.current">
                  <span class="dot"><mat-icon>{{ step.done ? 'check' : step.icon }}</mat-icon></span>
                  <div>
                    <strong>{{ step.label }}</strong>
                    @if (step.at) {
                      <span class="time">{{ step.at | date:'short' }}</span>
                    } @else if (step.current) {
                      <span class="time hint">En cours…</span>
                    }
                  </div>
                </li>
              }
            </ol>
            <p class="eta">
              <mat-icon>schedule</mat-icon>
              Livraison estimée&nbsp;: <strong>{{ eta() | date:'fullDate' }}</strong>
            </p>
          </article>

          <article class="card map-card">
            <h2>Position du livreur</h2>
            <div class="map-shell">
              <div #mapHost class="map"></div>
              @if (!mapReady()) {
                <div class="overlay">
                  <mat-icon>map</mat-icon>
                  <p>@if (ssr) { Carte indisponible côté serveur } @else { Chargement de la carte… }</p>
                </div>
              }
            </div>
            <p class="legend">
              <mat-icon class="farm">agriculture</mat-icon> Ferme éleveur
              <span class="sep">·</span>
              <mat-icon class="you">home</mat-icon> Adresse livraison
            </p>
          </article>
        </div>
      </div>
    </section>
  `,
  styles: [`
    :host { display: block; background: var(--faso-bg); min-height: 100vh; }
    .container {
      max-width: 1200px;
      margin: 0 auto;
      padding: var(--faso-space-6) var(--faso-space-4) var(--faso-space-12);
    }
    .back { color: var(--faso-text-muted); margin-left: calc(var(--faso-space-4) * -1); }
    .head {
      display: flex;
      justify-content: space-between;
      align-items: flex-start;
      gap: var(--faso-space-3);
      margin: var(--faso-space-2) 0 var(--faso-space-6);
      flex-wrap: wrap;
    }
    .head h1 { margin: 0; font-size: var(--faso-text-3xl); font-weight: var(--faso-weight-bold); }
    .head p { margin: 4px 0 0; color: var(--faso-text-muted); }

    .grid {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: var(--faso-space-5);
    }
    @media (max-width: 899px) { .grid { grid-template-columns: 1fr; } }

    .card {
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
      padding: var(--faso-space-5);
    }
    .card h2 {
      margin: 0 0 var(--faso-space-4);
      font-size: var(--faso-text-lg);
      font-weight: var(--faso-weight-semibold);
    }

    .timeline {
      list-style: none;
      padding: 0;
      margin: 0;
      position: relative;
    }
    .timeline::before {
      content: "";
      position: absolute;
      left: 19px;
      top: 16px;
      bottom: 16px;
      width: 2px;
      background: var(--faso-border);
    }
    .timeline li {
      position: relative;
      display: flex;
      align-items: center;
      gap: var(--faso-space-3);
      padding: 8px 0;
    }
    .dot {
      width: 40px; height: 40px;
      border-radius: 50%;
      background: var(--faso-surface-alt);
      border: 2px solid var(--faso-border);
      color: var(--faso-text-subtle);
      display: inline-flex;
      align-items: center;
      justify-content: center;
      z-index: 1;
      flex-shrink: 0;
    }
    .dot mat-icon { font-size: 20px; width: 20px; height: 20px; }

    .timeline li.done .dot {
      background: var(--faso-success);
      border-color: var(--faso-success);
      color: #FFFFFF;
    }
    .timeline li.current .dot {
      background: var(--faso-accent-500);
      border-color: var(--faso-accent-500);
      color: #FFFFFF;
      box-shadow: 0 0 0 4px var(--faso-accent-100);
    }

    .timeline strong {
      font-weight: var(--faso-weight-semibold);
      color: var(--faso-text);
      display: block;
    }
    .timeline .time {
      color: var(--faso-text-muted);
      font-size: var(--faso-text-sm);
    }
    .timeline .hint { color: var(--faso-accent-700); font-weight: var(--faso-weight-medium); }

    .eta {
      margin: var(--faso-space-4) 0 0;
      padding: var(--faso-space-3);
      background: var(--faso-primary-50);
      border-radius: var(--faso-radius-md);
      color: var(--faso-primary-700);
      display: flex;
      align-items: center;
      gap: 8px;
    }
    .eta mat-icon { font-size: 18px; width: 18px; height: 18px; }

    .map-shell {
      position: relative;
      aspect-ratio: 4 / 3;
      border-radius: var(--faso-radius-lg);
      overflow: hidden;
      border: 1px solid var(--faso-border);
      background: var(--faso-surface-alt);
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
      color: var(--faso-text-muted);
    }
    .overlay mat-icon { font-size: 40px; width: 40px; height: 40px; color: var(--faso-primary-400); }

    .legend {
      margin: var(--faso-space-3) 0 0;
      color: var(--faso-text-muted);
      font-size: var(--faso-text-sm);
      display: flex;
      align-items: center;
      gap: 4px;
    }
    .legend mat-icon { font-size: 16px; width: 16px; height: 16px; }
    .legend .farm { color: var(--faso-primary-600); }
    .legend .you { color: var(--faso-accent-700); }
    .legend .sep { margin: 0 6px; }
  `],
})
export class OrderTrackingComponent implements OnInit, OnDestroy {
  @ViewChild('mapHost', { static: true }) host!: ElementRef<HTMLDivElement>;

  private readonly route = inject(ActivatedRoute);
  private readonly platformId = inject(PLATFORM_ID);

  readonly ssr = !isPlatformBrowser(this.platformId);
  readonly mapReady = signal(false);
  readonly orderNumber = signal('');
  readonly createdAt = signal<string | undefined>(undefined);
  readonly currentStatus = signal('preparation');
  readonly eta = signal<string>(new Date(Date.now() + 2 * 86400000).toISOString());

  readonly timeline = signal<TimelineStep[]>([
    { key: 'placed',   label: 'Commande passée',     icon: 'assignment',      done: true,  at: new Date(Date.now() - 3 * 3600000).toISOString() },
    { key: 'confirmed',label: 'Confirmée éleveur',   icon: 'done_all',        done: true,  at: new Date(Date.now() - 2 * 3600000).toISOString() },
    { key: 'prep',     label: 'En préparation',      icon: 'inventory',       done: false, current: true },
    { key: 'shipping', label: 'En cours de livraison', icon: 'local_shipping',done: false },
    { key: 'delivered',label: 'Livrée',              icon: 'home',            done: false },
  ]);

  private map: any = null;

  async ngOnInit() {
    this.orderNumber.set(this.route.snapshot.paramMap.get('id') ?? 'CMD-XXXXXX');
    this.createdAt.set(new Date(Date.now() - 3 * 3600000).toISOString());

    if (!isPlatformBrowser(this.platformId)) return;

    // Ensure Leaflet CSS is loaded (CDN, once).
    if (!document.getElementById('leaflet-css')) {
      const link = document.createElement('link');
      link.id = 'leaflet-css';
      link.rel = 'stylesheet';
      link.href = 'https://unpkg.com/leaflet@1.9.4/dist/leaflet.css';
      document.head.appendChild(link);
    }

    const L = await import('leaflet');
    const iconOpts = {
      iconRetinaUrl: 'https://unpkg.com/leaflet@1.9.4/dist/images/marker-icon-2x.png',
      iconUrl: 'https://unpkg.com/leaflet@1.9.4/dist/images/marker-icon.png',
      shadowUrl: 'https://unpkg.com/leaflet@1.9.4/dist/images/marker-shadow.png',
      iconSize: [25, 41] as [number, number], iconAnchor: [12, 41] as [number, number],
    };
    (L.Marker.prototype as any).options.icon = L.icon(iconOpts);

    // Stub: farm at Koudougou, client in Ouagadougou
    const farm: [number, number] = [12.2530, -2.3622];
    const client: [number, number] = [12.3714, -1.5197];

    this.map = L.map(this.host.nativeElement, {
      center: [(farm[0] + client[0]) / 2, (farm[1] + client[1]) / 2],
      zoom: 8,
      scrollWheelZoom: false,
    });

    L.tileLayer('https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', {
      attribution: '&copy; OpenStreetMap',
      maxZoom: 16,
    }).addTo(this.map);

    L.marker(farm).bindPopup('Ferme éleveur').addTo(this.map);
    L.marker(client).bindPopup('Adresse livraison').addTo(this.map);

    L.polyline([farm, client], {
      color: '#2E7D32',
      weight: 4,
      opacity: 0.7,
      dashArray: '8,6',
    }).addTo(this.map);

    this.map.fitBounds([farm, client], { padding: [30, 30] });
    this.mapReady.set(true);
  }

  ngOnDestroy(): void {
    if (this.map) { this.map.remove(); this.map = null; }
  }
}
