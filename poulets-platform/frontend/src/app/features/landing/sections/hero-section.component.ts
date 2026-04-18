import { ChangeDetectionStrategy, Component, inject } from '@angular/core';
import { CommonModule } from '@angular/common';
import { Router } from '@angular/router';
import { TranslateModule } from '@ngx-translate/core';
import { SearchHeroComponent, SearchQuery } from '@shared/components/search-hero/search-hero.component';
import { TrustBadgeComponent } from '@shared/components/trust-badge/trust-badge.component';
import { CHICKEN_RACES } from '@shared/models/marketplace.models';

@Component({
  selector: 'app-landing-hero',
  standalone: true,
  imports: [CommonModule, TranslateModule, SearchHeroComponent, TrustBadgeComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="hero" id="accueil">
      <div class="hero-bg" aria-hidden="true">
        <img src="assets/img/hero-farm-illustration.svg" alt="">
        <div class="hero-overlay"></div>
      </div>
      <div class="hero-inner">
        <div class="hero-copy">
          <div class="hero-badges">
            <app-trust-badge kind="flag" />
            <app-trust-badge kind="halal" />
            <app-trust-badge kind="local" label="Direct producteur" />
          </div>
          <h1>{{ 'landing.hero2.title' | translate }}</h1>
          <p class="lead">{{ 'landing.hero2.lead' | translate }}</p>
        </div>

        <div class="hero-search">
          <app-search-hero [races]="races" (search)="onSearch($event)" />
          <div class="hero-hints">
            <span>{{ 'landing.hero2.hint1' | translate }}</span>
            <span aria-hidden="true">·</span>
            <span>{{ 'landing.hero2.hint2' | translate }}</span>
          </div>
        </div>
      </div>
    </section>
  `,
  styles: [`
    .hero {
      position: relative;
      min-height: clamp(520px, 80vh, 720px);
      display: flex;
      align-items: center;
      justify-content: center;
      padding: var(--faso-space-10) var(--faso-space-4) var(--faso-space-12);
      overflow: hidden;
    }
    .hero-bg {
      position: absolute;
      inset: 0;
      z-index: 0;
    }
    .hero-bg img {
      width: 100%;
      height: 100%;
      object-fit: cover;
    }
    .hero-overlay {
      position: absolute;
      inset: 0;
      background: linear-gradient(180deg,
        rgba(15, 62, 30, 0.15) 0%,
        rgba(15, 62, 30, 0.45) 55%,
        rgba(15, 62, 30, 0.70) 100%
      );
    }
    .hero-inner {
      position: relative;
      z-index: 1;
      width: 100%;
      max-width: 1100px;
      display: flex;
      flex-direction: column;
      gap: var(--faso-space-8);
      align-items: center;
      text-align: center;
      color: #FFFFFF;
    }
    .hero-badges {
      display: flex;
      flex-wrap: wrap;
      gap: var(--faso-space-2);
      justify-content: center;
    }
    .hero-copy h1 {
      font-size: clamp(2rem, 5vw, 3.5rem);
      font-weight: var(--faso-weight-bold);
      line-height: 1.1;
      letter-spacing: -0.02em;
      color: inherit;
      text-shadow: 0 2px 12px rgba(0,0,0,0.25);
      max-width: 24ch;
      margin-inline: auto;
    }
    .hero-copy .lead {
      margin-top: var(--faso-space-4);
      font-size: var(--faso-text-xl);
      opacity: 0.95;
      max-width: 52ch;
      margin-inline: auto;
    }
    .hero-search { width: 100%; }
    .hero-hints {
      margin-top: var(--faso-space-3);
      display: flex;
      gap: 6px;
      justify-content: center;
      font-size: var(--faso-text-sm);
      color: rgba(255,255,255,0.92);
    }

    @media (max-width: 767px) {
      .hero-copy .lead { font-size: var(--faso-text-base); }
    }
  `],
})
export class LandingHeroComponent {
  private router = inject(Router);
  readonly races = CHICKEN_RACES;

  onSearch(q: SearchQuery) {
    this.router.navigate(['/marketplace/annonces'], { queryParams: {
      race: q.race || null,
      location: q.location || null,
      date: q.date || null,
    }});
  }
}
