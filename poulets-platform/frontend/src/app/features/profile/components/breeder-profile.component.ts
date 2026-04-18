// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, OnInit, inject, signal, computed } from '@angular/core';
import { CommonModule, DatePipe } from '@angular/common';
import { ActivatedRoute, RouterLink } from '@angular/router';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';

import { BreederAvatarComponent } from '@shared/components/breeder-avatar/breeder-avatar.component';
import { TrustBadgeComponent, TrustBadgeKind } from '@shared/components/trust-badge/trust-badge.component';
import { RatingStarsComponent } from '@shared/components/rating-stars/rating-stars.component';
import { SectionHeaderComponent } from '@shared/components/section-header/section-header.component';
import { EmptyStateComponent } from '@shared/components/empty-state/empty-state.component';
import { ReviewSummaryComponent } from '@features/reputation/components/review-summary.component';
import { ReviewListComponent } from '@features/reputation/components/review-list.component';

import { BreederProfileService } from '../services/breeder-profile.service';
import { ReputationService } from '@features/reputation/services/reputation.service';
import { BreederProfile, ReviewStats } from '@shared/models/reputation.models';

@Component({
  selector: 'app-breeder-profile',
  standalone: true,
  imports: [
    CommonModule, RouterLink, DatePipe,
    MatIconModule, MatButtonModule,
    BreederAvatarComponent, TrustBadgeComponent, RatingStarsComponent,
    SectionHeaderComponent, EmptyStateComponent,
    ReviewSummaryComponent, ReviewListComponent,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    @if (profile(); as p) {
      <article class="page">
        <header class="hero" [style.background-image]="coverUrl(p)">
          <div class="hero-overlay"></div>
          <div class="hero-inner">
            <app-breeder-avatar size="xl" [name]="fullName(p)" [photo]="p.avatar || null" [verified]="p.veterinaryVerified" />
            <div class="identity">
              <h1>{{ fullName(p) }}</h1>
              <p class="meta">
                <mat-icon>location_on</mat-icon>
                {{ p.city }}, {{ p.region }}
                @if (p.distanceKm != null) { · {{ p.distanceKm }} km }
              </p>
              <div class="chips">
                @for (b of trustBadges(p); track b) {
                  <app-trust-badge [kind]="b" />
                }
              </div>
            </div>
            <div class="cta">
              <a mat-raised-button color="primary" [routerLink]="['/messaging/new']" [queryParams]="{ to: p.id }">
                <mat-icon>chat</mat-icon> Contacter
              </a>
              @if (p.whatsapp) {
                <a mat-button [href]="'https://wa.me/' + waNumber(p.whatsapp)" target="_blank" rel="noopener">
                  <mat-icon>share</mat-icon> WhatsApp
                </a>
              }
            </div>
          </div>
        </header>

        <div class="container">
          <section class="kpis">
            <div class="kpi">
              <strong>
                @if (stats(); as s) { <app-rating-stars [value]="s.average" [showValue]="true" /> }
              </strong>
              <span>{{ stats()?.total || 0 }} avis</span>
            </div>
            <div class="kpi">
              <strong>{{ p.totalSales || 0 }}</strong>
              <span>ventes réalisées</span>
            </div>
            <div class="kpi">
              <strong>
                @if (p.responseTimeHours != null) {
                  &lt; {{ p.responseTimeHours }}h
                } @else { — }
              </strong>
              <span>délai de réponse</span>
            </div>
            <div class="kpi">
              <strong>{{ p.memberSince | date:'MMM y' }}</strong>
              <span>membre depuis</span>
            </div>
          </section>

          @if (p.bio) {
            <section class="bloc">
              <app-section-header title="À propos" />
              <p class="bio">{{ p.bio }}</p>
            </section>
          }

          @if (p.specialties.length > 0) {
            <section class="bloc">
              <app-section-header title="Spécialités" />
              <div class="pills">
                @for (sp of p.specialties; track sp) {
                  <span class="pill">{{ sp }}</span>
                }
              </div>
            </section>
          }

          <section class="bloc">
            <app-section-header title="Certifications" />
            <ul class="cert">
              <li [class.on]="p.halalCertified">
                <mat-icon>{{ p.halalCertified ? 'check_circle' : 'radio_button_unchecked' }}</mat-icon>
                Halal certifié
              </li>
              <li [class.on]="p.veterinaryVerified">
                <mat-icon>{{ p.veterinaryVerified ? 'check_circle' : 'radio_button_unchecked' }}</mat-icon>
                Vétérinaire vérifié
              </li>
              <li [class.on]="p.bioCertified">
                <mat-icon>{{ p.bioCertified ? 'check_circle' : 'radio_button_unchecked' }}</mat-icon>
                Production biologique
              </li>
            </ul>
          </section>

          <section class="bloc">
            <app-section-header title="Galerie" />
            @if (p.gallery?.length) {
              <div class="gallery">
                @for (g of p.gallery; track g) {
                  <img [src]="g" alt="Photo de ferme" loading="lazy">
                }
              </div>
            } @else {
              <app-empty-state icon="photo_library" title="profile.breeder.gallery.empty">
                <p>L'éleveur n'a pas encore publié de photos de sa ferme.</p>
              </app-empty-state>
            }
          </section>

          <section class="bloc" id="avis">
            <app-section-header title="Avis clients" kicker="Retours vérifiés" />
            @if (stats(); as s) {
              @if (s.total > 0) {
                <app-review-summary [stats]="s" />
              }
            }
            <div class="review-wrap">
              <app-review-list [breederId]="p.id" />
            </div>
          </section>
        </div>
      </article>
    } @else if (notFound()) {
      <app-empty-state icon="person_off" title="profile.breeder.notFound">
        <a mat-raised-button color="primary" routerLink="/marketplace/annonces">
          Retour au marketplace
        </a>
      </app-empty-state>
    } @else {
      <div class="loading">Chargement…</div>
    }
  `,
  styles: [`
    :host { display: block; background: var(--faso-bg); min-height: 100vh; }

    .hero {
      position: relative;
      min-height: 260px;
      padding: var(--faso-space-16) var(--faso-space-4) var(--faso-space-8);
      background: var(--faso-gradient-brand);
      background-size: cover;
      background-position: center;
      color: #FFFFFF;
    }
    .hero-overlay {
      position: absolute;
      inset: 0;
      background: linear-gradient(180deg, rgba(15,62,30,0.20) 0%, rgba(15,62,30,0.55) 100%);
    }
    .hero-inner {
      position: relative;
      max-width: 1200px;
      margin: 0 auto;
      display: grid;
      grid-template-columns: auto 1fr auto;
      gap: var(--faso-space-5);
      align-items: end;
    }
    .identity h1 {
      margin: 0;
      color: inherit;
      font-size: clamp(1.75rem, 3vw, 2.25rem);
      font-weight: var(--faso-weight-bold);
      text-shadow: 0 2px 12px rgba(0,0,0,0.25);
    }
    .meta {
      display: inline-flex;
      align-items: center;
      gap: 4px;
      margin: 4px 0 var(--faso-space-3);
      opacity: 0.95;
    }
    .meta mat-icon { font-size: 18px; width: 18px; height: 18px; }
    .chips {
      display: flex;
      flex-wrap: wrap;
      gap: 6px;
    }
    .cta { display: flex; gap: var(--faso-space-2); }
    .cta a { white-space: nowrap; }

    .container {
      max-width: 1100px;
      margin: calc(var(--faso-space-8) * -1) auto 0;
      padding: 0 var(--faso-space-4) var(--faso-space-12);
      position: relative;
    }

    .kpis {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
      gap: var(--faso-space-4);
      padding: var(--faso-space-5);
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
      box-shadow: var(--faso-shadow-md);
      margin-bottom: var(--faso-space-8);
    }
    .kpi {
      display: flex;
      flex-direction: column;
      gap: 2px;
      text-align: center;
    }
    .kpi strong {
      font-size: var(--faso-text-xl);
      font-weight: var(--faso-weight-semibold);
      color: var(--faso-primary-700);
      line-height: 1.2;
    }
    .kpi span {
      color: var(--faso-text-muted);
      font-size: var(--faso-text-sm);
    }

    .bloc {
      margin-bottom: var(--faso-space-10);
    }
    .bio {
      color: var(--faso-text);
      line-height: var(--faso-leading-relaxed);
      max-width: 70ch;
      margin: 0;
    }

    .pills {
      display: flex;
      flex-wrap: wrap;
      gap: 6px;
    }
    .pill {
      padding: 4px 12px;
      background: var(--faso-primary-50);
      color: var(--faso-primary-700);
      border: 1px solid var(--faso-primary-200);
      border-radius: var(--faso-radius-pill);
      font-size: var(--faso-text-sm);
      font-weight: var(--faso-weight-medium);
    }

    .cert {
      list-style: none;
      padding: 0;
      margin: 0;
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
      gap: var(--faso-space-3);
    }
    .cert li {
      display: inline-flex;
      align-items: center;
      gap: 8px;
      padding: var(--faso-space-3) var(--faso-space-4);
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-lg);
      color: var(--faso-text-muted);
    }
    .cert li mat-icon { color: var(--faso-text-subtle); }
    .cert li.on {
      border-color: var(--faso-success);
      background: var(--faso-success-bg);
      color: var(--faso-text);
    }
    .cert li.on mat-icon { color: var(--faso-success); }

    .gallery {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
      gap: var(--faso-space-3);
    }
    .gallery img {
      aspect-ratio: 4/3;
      object-fit: cover;
      border-radius: var(--faso-radius-lg);
      border: 1px solid var(--faso-border);
    }

    .review-wrap {
      margin-top: var(--faso-space-5);
      padding: var(--faso-space-5);
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
    }

    .loading {
      padding: var(--faso-space-16) var(--faso-space-4);
      text-align: center;
      color: var(--faso-text-muted);
    }

    @media (max-width: 767px) {
      .hero-inner {
        grid-template-columns: 1fr;
        gap: var(--faso-space-3);
      }
      .cta { justify-content: flex-start; }
    }
  `],
})
export class BreederProfileComponent implements OnInit {
  private readonly route = inject(ActivatedRoute);
  private readonly breederSvc = inject(BreederProfileService);
  private readonly repSvc = inject(ReputationService);

  readonly profile = signal<BreederProfile | null>(null);
  readonly stats = signal<ReviewStats | null>(null);
  readonly notFound = signal(false);

  ngOnInit(): void {
    const id = this.route.snapshot.paramMap.get('id');
    if (!id) { this.notFound.set(true); return; }

    this.breederSvc.getById(id).subscribe((p) => {
      if (!p) { this.notFound.set(true); return; }
      this.profile.set(p);
    });
    this.repSvc.getStats(id).subscribe((s) => this.stats.set(s));
  }

  fullName(p: BreederProfile): string {
    return p.prenom ? `${p.prenom} ${p.name}` : p.name;
  }

  coverUrl(p: BreederProfile): string {
    return p.coverPhoto ? `url(${p.coverPhoto})` : '';
  }

  trustBadges(p: BreederProfile): TrustBadgeKind[] {
    const out: TrustBadgeKind[] = [];
    if (p.halalCertified) out.push('halal');
    if (p.veterinaryVerified) out.push('vet');
    if (p.bioCertified) out.push('bio');
    out.push('flag');
    return out;
  }

  waNumber(n: string): string {
    return n.replace(/\D/g, '');
  }
}
