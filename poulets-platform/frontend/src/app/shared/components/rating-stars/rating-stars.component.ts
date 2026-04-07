import { Component, Input, Output, EventEmitter, ChangeDetectionStrategy } from '@angular/core';
import { CommonModule } from '@angular/common';
import { MatIconModule } from '@angular/material/icon';

@Component({
  selector: 'app-rating-stars',
  standalone: true,
  imports: [CommonModule, MatIconModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="rating-stars" [class.interactive]="interactive">
      @for (star of stars; track star) {
        <mat-icon
          class="star"
          [class.filled]="star <= filledStars"
          [class.half]="star === filledStars + 1 && hasHalf"
          (click)="onStarClick(star)"
          (mouseenter)="onHover(star)"
          (mouseleave)="onLeave()"
        >
          {{ getStarIcon(star) }}
        </mat-icon>
      }
      @if (showValue) {
        <span class="rating-value">{{ displayValue }}</span>
      }
      @if (showCount && count > 0) {
        <span class="rating-count">({{ count }})</span>
      }
    </div>
  `,
  styles: [`
    .rating-stars {
      display: inline-flex;
      align-items: center;
      gap: 2px;
    }

    .star {
      font-size: 20px;
      width: 20px;
      height: 20px;
      color: #e0e0e0;
      transition: color 0.15s ease;
    }

    .star.filled {
      color: #ff9800;
    }

    .star.half {
      color: #ff9800;
    }

    .interactive .star {
      cursor: pointer;
    }

    .interactive .star:hover {
      color: #ffb74d;
    }

    .rating-value {
      margin-left: 8px;
      font-weight: 500;
      font-size: 0.9rem;
      color: var(--faso-text);
    }

    .rating-count {
      margin-left: 4px;
      font-size: 0.8rem;
      color: var(--faso-text-secondary);
    }
  `],
})
export class RatingStarsComponent {
  @Input() value = 0;
  @Input() maxStars = 5;
  @Input() interactive = false;
  @Input() showValue = false;
  @Input() showCount = false;
  @Input() count = 0;
  @Output() ratingChange = new EventEmitter<number>();

  hoveredStar = 0;

  get stars(): number[] {
    return Array.from({ length: this.maxStars }, (_, i) => i + 1);
  }

  get currentValue(): number {
    return this.hoveredStar || this.value;
  }

  get filledStars(): number {
    return Math.floor(this.currentValue);
  }

  get hasHalf(): boolean {
    return this.currentValue % 1 >= 0.5;
  }

  get displayValue(): string {
    return this.value.toFixed(1);
  }

  getStarIcon(star: number): string {
    if (star <= this.filledStars) {
      return 'star';
    }
    if (star === this.filledStars + 1 && this.hasHalf) {
      return 'star_half';
    }
    return 'star_border';
  }

  onStarClick(star: number): void {
    if (this.interactive) {
      this.value = star;
      this.ratingChange.emit(star);
    }
  }

  onHover(star: number): void {
    if (this.interactive) {
      this.hoveredStar = star;
    }
  }

  onLeave(): void {
    this.hoveredStar = 0;
  }
}
