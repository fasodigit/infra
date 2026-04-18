// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, Input, OnChanges, SimpleChanges, inject, signal } from '@angular/core';
import { CommonModule, DatePipe } from '@angular/common';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';
import { BreederAvatarComponent } from '@shared/components/breeder-avatar/breeder-avatar.component';
import { RatingStarsComponent } from '@shared/components/rating-stars/rating-stars.component';
import { Review } from '@shared/models/reputation.models';
import { ReputationService } from '../services/reputation.service';

type SortKey = 'recent' | 'best';

@Component({
  selector: 'app-review-list',
  standalone: true,
  imports: [
    CommonModule, DatePipe, MatIconModule, MatButtonModule,
    BreederAvatarComponent, RatingStarsComponent,
  ],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="list" aria-label="Avis clients">
      <header class="head">
        <h3>{{ total() }} avis clients</h3>
        <div class="controls">
          <label for="reviewSort" class="visually-hidden">Trier les avis</label>
          <select id="reviewSort" (change)="onSortChange($event)">
            <option value="recent" [selected]="sortKey === 'recent'">Plus récents</option>
            <option value="best" [selected]="sortKey === 'best'">Mieux notés</option>
          </select>
        </div>
      </header>

      @if (loading()) {
        <p class="loading">Chargement des avis…</p>
      } @else if (visible().length === 0) {
        <p class="empty">Aucun avis pour le moment. Soyez le premier à commenter&nbsp;!</p>
      } @else {
        <ul class="items">
          @for (r of visible(); track r.id) {
            <li class="review">
              <app-breeder-avatar size="md" [name]="r.clientName" [photo]="r.clientAvatar || null" />
              <div class="body">
                <div class="top">
                  <strong>{{ r.clientName }}</strong>
                  <time>{{ r.createdAt | date:'mediumDate' }}</time>
                </div>
                <app-rating-stars [value]="r.rating" />
                <p class="comment">{{ r.comment }}</p>
                @if (r.photos?.length) {
                  <div class="photos">
                    @for (ph of r.photos; track ph) {
                      <img [src]="ph" alt="" loading="lazy">
                    }
                  </div>
                }
              </div>
            </li>
          }
        </ul>

        @if (hasMore()) {
          <div class="more">
            <button mat-button type="button" (click)="loadMore()">Voir plus d'avis</button>
          </div>
        }
      }
    </section>
  `,
  styles: [`
    :host { display: block; }
    .head {
      display: flex;
      align-items: center;
      justify-content: space-between;
      margin-bottom: var(--faso-space-4);
      gap: var(--faso-space-3);
    }
    h3 {
      margin: 0;
      font-size: var(--faso-text-xl);
      font-weight: var(--faso-weight-semibold);
    }
    .controls select {
      padding: 6px 10px;
      border: 1px solid var(--faso-border-strong);
      border-radius: var(--faso-radius-pill);
      background: var(--faso-surface);
      font-family: inherit;
    }
    .visually-hidden {
      position: absolute;
      width: 1px; height: 1px; overflow: hidden;
      clip: rect(0,0,0,0); white-space: nowrap;
    }
    .items {
      list-style: none;
      padding: 0;
      margin: 0;
      display: flex;
      flex-direction: column;
      gap: var(--faso-space-5);
    }
    .review {
      display: grid;
      grid-template-columns: auto 1fr;
      gap: var(--faso-space-3);
      padding-bottom: var(--faso-space-5);
      border-bottom: 1px solid var(--faso-border);
    }
    .review:last-child { border-bottom: none; padding-bottom: 0; }
    .body { display: flex; flex-direction: column; gap: 4px; }
    .top {
      display: flex;
      align-items: center;
      gap: var(--faso-space-3);
      flex-wrap: wrap;
    }
    .top strong { font-size: var(--faso-text-base); }
    .top time {
      color: var(--faso-text-subtle);
      font-size: var(--faso-text-sm);
    }
    .comment {
      margin: 4px 0 0;
      color: var(--faso-text);
      line-height: var(--faso-leading-relaxed);
    }
    .photos {
      display: flex;
      gap: 6px;
      margin-top: 6px;
      flex-wrap: wrap;
    }
    .photos img {
      width: 72px; height: 72px;
      object-fit: cover;
      border-radius: var(--faso-radius-md);
      border: 1px solid var(--faso-border);
    }
    .empty, .loading {
      color: var(--faso-text-muted);
      padding: var(--faso-space-6) 0;
      text-align: center;
    }
    .more { text-align: center; margin-top: var(--faso-space-4); }
  `],
})
export class ReviewListComponent implements OnChanges {
  @Input({ required: true }) breederId!: string;
  @Input() pageSize = 5;

  private readonly svc = inject(ReputationService);
  readonly loading = signal(true);
  readonly reviews = signal<Review[]>([]);
  readonly total = signal(0);
  readonly visibleCount = signal(0);
  sortKey: SortKey = 'recent';

  readonly visible = () => {
    const n = this.visibleCount();
    const arr = this.sortReviews(this.reviews());
    return arr.slice(0, n);
  };

  readonly hasMore = () => this.visibleCount() < this.reviews().length;

  ngOnChanges(changes: SimpleChanges): void {
    if (changes['breederId']) this.load();
  }

  onSortChange(ev: Event) {
    this.sortKey = (ev.target as HTMLSelectElement).value as SortKey;
  }

  loadMore() {
    this.visibleCount.update(n => Math.min(n + this.pageSize, this.reviews().length));
  }

  private load() {
    this.loading.set(true);
    this.svc.getReviewsForBreeder(this.breederId, 0, 100).subscribe({
      next: (page) => {
        this.reviews.set(page.content);
        this.total.set(page.totalElements);
        this.visibleCount.set(this.pageSize);
        this.loading.set(false);
      },
      error: () => {
        this.reviews.set([]);
        this.total.set(0);
        this.loading.set(false);
      },
    });
  }

  private sortReviews(arr: Review[]): Review[] {
    const copy = [...arr];
    if (this.sortKey === 'best') {
      copy.sort((a, b) => b.rating - a.rating || b.createdAt.localeCompare(a.createdAt));
    } else {
      copy.sort((a, b) => b.createdAt.localeCompare(a.createdAt));
    }
    return copy;
  }
}
