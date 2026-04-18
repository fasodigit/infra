import { ChangeDetectionStrategy, Component, Input } from '@angular/core';
import { CommonModule } from '@angular/common';
import { MatIconModule } from '@angular/material/icon';

type AvatarSize = 'sm' | 'md' | 'lg' | 'xl';

@Component({
  selector: 'app-breeder-avatar',
  standalone: true,
  imports: [CommonModule, MatIconModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <span class="avatar" [class]="'avatar--' + size" [attr.aria-label]="name">
      <img
        [src]="photo || fallback"
        [alt]="name"
        loading="lazy"
        (error)="onError($event)"
      >
      @if (verified) {
        <mat-icon class="badge" aria-label="Éleveur vérifié">verified</mat-icon>
      }
    </span>
  `,
  styles: [`
    :host { display: inline-flex; }

    .avatar {
      position: relative;
      display: inline-flex;
      border-radius: 50%;
      overflow: visible;
      background: var(--faso-primary-50);
      flex-shrink: 0;
    }

    .avatar img {
      width: 100%;
      height: 100%;
      border-radius: 50%;
      object-fit: cover;
      display: block;
    }

    .avatar--sm { width: 28px; height: 28px; }
    .avatar--md { width: 40px; height: 40px; }
    .avatar--lg { width: 64px; height: 64px; }
    .avatar--xl { width: 96px; height: 96px; }

    .badge {
      position: absolute;
      right: -2px;
      bottom: -2px;
      width: 18px;
      height: 18px;
      font-size: 18px;
      color: var(--faso-accent-700);
      background: var(--faso-surface);
      border-radius: 50%;
      box-shadow: var(--faso-shadow-xs);
    }

    .avatar--lg .badge, .avatar--xl .badge {
      width: 24px; height: 24px; font-size: 24px;
    }
  `],
})
export class BreederAvatarComponent {
  @Input() name = '';
  @Input() photo: string | null = null;
  @Input() size: AvatarSize = 'md';
  @Input() verified = false;

  readonly fallback = 'assets/img/placeholder-eleveur.svg';

  onError(ev: Event) {
    (ev.target as HTMLImageElement).src = this.fallback;
  }
}
