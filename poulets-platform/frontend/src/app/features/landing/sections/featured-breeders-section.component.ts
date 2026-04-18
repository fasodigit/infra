import { ChangeDetectionStrategy, Component } from '@angular/core';
import { CommonModule } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatIconModule } from '@angular/material/icon';
import { TranslateModule } from '@ngx-translate/core';
import { SectionHeaderComponent } from '@shared/components/section-header/section-header.component';
import { BreederAvatarComponent } from '@shared/components/breeder-avatar/breeder-avatar.component';
import { RatingStarsComponent } from '@shared/components/rating-stars/rating-stars.component';

interface FeaturedBreeder {
  id: string;
  name: string;
  region: string;
  specialty: string;
  quote: string;
  rating: number;
  reviewCount: number;
  verified: boolean;
}

@Component({
  selector: 'app-landing-featured-breeders',
  standalone: true,
  imports: [
    CommonModule, RouterLink, MatIconModule, TranslateModule,
    SectionHeaderComponent, BreederAvatarComponent, RatingStarsComponent,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="wrap" id="eleveurs">
      <div class="inner">
        <app-section-header
          kicker="Communauté"
          [title]="'landing.featured.title' | translate"
          [subtitle]="'landing.featured.subtitle' | translate"
        />
        <div class="grid">
          @for (b of breeders; track b.id) {
            <article class="card">
              <div class="head">
                <app-breeder-avatar size="lg" [name]="b.name" [verified]="b.verified" />
                <div>
                  <h3>{{ b.name }}</h3>
                  <p class="region"><mat-icon>location_on</mat-icon> {{ b.region }}</p>
                </div>
              </div>
              <app-rating-stars [value]="b.rating" [showValue]="true" [showCount]="true" [count]="b.reviewCount" />
              <p class="specialty"><mat-icon>verified</mat-icon> {{ b.specialty }}</p>
              <blockquote>« {{ b.quote }} »</blockquote>
            </article>
          }
        </div>
      </div>
    </section>
  `,
  styles: [`
    .wrap {
      background: var(--faso-surface);
      padding: var(--faso-space-12) var(--faso-space-4);
    }
    .inner { max-width: 1200px; margin-inline: auto; }
    .grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
      gap: var(--faso-space-6);
    }
    .card {
      padding: var(--faso-space-6);
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
      box-shadow: var(--faso-elevation-card);
    }
    .head {
      display: flex;
      align-items: center;
      gap: var(--faso-space-3);
      margin-bottom: var(--faso-space-4);
    }
    h3 {
      margin: 0;
      font-size: var(--faso-text-lg);
      font-weight: var(--faso-weight-semibold);
    }
    .region {
      display: inline-flex; align-items: center; gap: 4px;
      margin: 2px 0 0;
      color: var(--faso-text-muted);
      font-size: var(--faso-text-sm);
    }
    .region mat-icon { font-size: 14px; width: 14px; height: 14px; }
    .specialty {
      display: inline-flex; align-items: center; gap: 4px;
      margin: var(--faso-space-3) 0 0;
      color: var(--faso-primary-700);
      font-size: var(--faso-text-sm);
      font-weight: var(--faso-weight-medium);
    }
    .specialty mat-icon { font-size: 16px; width: 16px; height: 16px; color: var(--faso-accent-700); }
    blockquote {
      margin: var(--faso-space-3) 0 0;
      padding-left: var(--faso-space-4);
      border-left: 3px solid var(--faso-accent-500);
      color: var(--faso-text-muted);
      font-style: italic;
      font-size: var(--faso-text-sm);
      line-height: var(--faso-leading-relaxed);
    }
  `],
})
export class LandingFeaturedBreedersComponent {
  readonly breeders: FeaturedBreeder[] = [
    {
      id: '1', name: 'Kassim Ouédraogo', region: 'Ouagadougou',
      specialty: 'Poulet bicyclette · Halal',
      quote: 'Chaque lot est suivi du premier jour jusqu\'à la livraison. C\'est notre fierté.',
      rating: 4.8, reviewCount: 142, verified: true,
    },
    {
      id: '2', name: 'Awa Sankara', region: 'Bobo-Dioulasso',
      specialty: 'Pondeuses bio',
      quote: 'Mes clients apprécient la fraîcheur et la régularité de mes livraisons.',
      rating: 4.9, reviewCount: 98, verified: true,
    },
    {
      id: '3', name: 'Oumar Traoré', region: 'Koudougou',
      specialty: 'Coopérative 12 éleveurs',
      quote: 'Ensemble, nous pouvons livrer de plus gros volumes sans perdre la qualité.',
      rating: 4.7, reviewCount: 76, verified: true,
    },
  ];
}
