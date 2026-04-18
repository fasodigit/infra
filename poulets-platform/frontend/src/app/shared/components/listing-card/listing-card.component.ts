import { ChangeDetectionStrategy, Component, Input } from '@angular/core';
import { CommonModule, DecimalPipe } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatIconModule } from '@angular/material/icon';
import { RatingStarsComponent } from '../rating-stars/rating-stars.component';
import { TrustBadgeComponent, TrustBadgeKind } from '../trust-badge/trust-badge.component';

@Component({
  selector: 'app-listing-card',
  standalone: true,
  imports: [CommonModule, RouterLink, MatIconModule, RatingStarsComponent, TrustBadgeComponent, DecimalPipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <a class="card" [routerLink]="routerLink" [attr.aria-label]="title">
      <div class="media">
        <img
          [src]="photo || fallback"
          [alt]="title"
          loading="lazy"
          (error)="onImgError($event)"
        >
        @if (badges.length > 0) {
          <div class="badges">
            @for (b of badges; track b) {
              <app-trust-badge [kind]="b" />
            }
          </div>
        }
        @if (favorite !== undefined) {
          <button
            type="button"
            class="fav"
            [attr.aria-label]="favorite ? 'Retirer des favoris' : 'Ajouter aux favoris'"
            [attr.aria-pressed]="favorite"
            (click)="onFav($event)"
          >
            <mat-icon>{{ favorite ? 'favorite' : 'favorite_border' }}</mat-icon>
          </button>
        }
      </div>

      <div class="body">
        <div class="head">
          <h3 class="title">{{ title }}</h3>
          @if (location) {
            <span class="loc">
              <mat-icon>location_on</mat-icon>
              {{ location }}@if (distanceKm != null) { · {{ distanceKm | number:'1.0-0' }} km }
            </span>
          }
        </div>

        @if (breederName) {
          <div class="breeder">
            <span class="name">{{ breederName }}</span>
            @if (rating != null) {
              <app-rating-stars
                [value]="rating"
                [showValue]="true"
                [showCount]="!!reviewCount"
                [count]="reviewCount || 0"
              />
            }
          </div>
        }

        @if (subtitle) { <p class="sub">{{ subtitle }}</p> }

        @if (priceValue != null) {
          <div class="price">
            <strong>{{ priceValue | number:'1.0-0' }}</strong>
            <span>{{ priceLabel || 'FCFA' }}</span>
          </div>
        }
      </div>
    </a>
  `,
  styles: [`
    :host { display: block; height: 100%; }
    .card {
      display: flex;
      flex-direction: column;
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
      overflow: hidden;
      text-decoration: none;
      color: inherit;
      height: 100%;
      transition:
        transform var(--faso-duration-fast) var(--faso-ease-standard),
        box-shadow var(--faso-duration-fast) var(--faso-ease-standard),
        border-color var(--faso-duration-fast) var(--faso-ease-standard);
    }
    .card:hover, .card:focus-visible {
      transform: translateY(-3px);
      box-shadow: var(--faso-elevation-hover);
      border-color: var(--faso-primary-200);
      text-decoration: none;
    }
    @media (prefers-reduced-motion: reduce) {
      .card:hover, .card:focus-visible { transform: none; }
    }

    .media {
      position: relative;
      aspect-ratio: 16 / 10;
      background: var(--faso-primary-50);
      overflow: hidden;
    }
    .media img {
      width: 100%;
      height: 100%;
      object-fit: cover;
      transition: transform var(--faso-duration-slow) var(--faso-ease-standard);
    }
    .card:hover .media img { transform: scale(1.04); }

    .badges {
      position: absolute;
      top: var(--faso-space-2);
      left: var(--faso-space-2);
      display: flex;
      flex-wrap: wrap;
      gap: 4px;
    }

    .fav {
      position: absolute;
      top: var(--faso-space-2);
      right: var(--faso-space-2);
      width: 36px; height: 36px;
      border-radius: 50%;
      background: rgba(255,255,255,0.92);
      color: var(--faso-danger);
      border: none;
      display: inline-flex;
      align-items: center;
      justify-content: center;
      cursor: pointer;
      box-shadow: var(--faso-shadow-xs);
    }
    .fav mat-icon { font-size: 20px; width: 20px; height: 20px; }

    .body {
      padding: var(--faso-space-4);
      display: flex;
      flex-direction: column;
      gap: var(--faso-space-2);
      flex: 1;
    }
    .head { display: flex; flex-direction: column; gap: 2px; }
    .title {
      margin: 0;
      font-size: var(--faso-text-lg);
      font-weight: var(--faso-weight-semibold);
      line-height: 1.3;
      display: -webkit-box;
      -webkit-line-clamp: 2;
      -webkit-box-orient: vertical;
      overflow: hidden;
    }
    .loc {
      display: inline-flex;
      align-items: center;
      gap: 2px;
      color: var(--faso-text-muted);
      font-size: var(--faso-text-sm);
    }
    .loc mat-icon { font-size: 14px; width: 14px; height: 14px; }

    .breeder {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: var(--faso-space-2);
    }
    .name {
      color: var(--faso-text-muted);
      font-size: var(--faso-text-sm);
      font-weight: var(--faso-weight-medium);
    }

    .sub {
      margin: 0;
      color: var(--faso-text-muted);
      font-size: var(--faso-text-sm);
      display: -webkit-box;
      -webkit-line-clamp: 2;
      -webkit-box-orient: vertical;
      overflow: hidden;
    }

    .price {
      margin-top: auto;
      display: flex;
      align-items: baseline;
      gap: 4px;
      color: var(--faso-primary-700);
    }
    .price strong {
      font-size: var(--faso-text-xl);
      font-weight: var(--faso-weight-bold);
    }
    .price span {
      font-size: var(--faso-text-sm);
      color: var(--faso-text-muted);
    }
  `],
})
export class ListingCardComponent {
  @Input({ required: true }) title!: string;
  @Input() subtitle?: string;
  @Input() photo: string | null | undefined = null;
  @Input() routerLink: string | any[] = '.';
  @Input() location?: string;
  @Input() distanceKm?: number;
  @Input() breederName?: string;
  @Input() rating?: number;
  @Input() reviewCount?: number;
  @Input() priceValue?: number;
  @Input() priceLabel?: string;
  @Input() badges: TrustBadgeKind[] = [];
  @Input() favorite?: boolean;

  readonly fallback = 'assets/img/placeholder-poulet.svg';

  onImgError(ev: Event) {
    (ev.target as HTMLImageElement).src = this.fallback;
  }

  onFav(ev: Event) {
    ev.preventDefault();
    ev.stopPropagation();
    this.favorite = !this.favorite;
  }
}
