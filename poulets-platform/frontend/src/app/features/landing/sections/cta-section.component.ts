import { ChangeDetectionStrategy, Component } from '@angular/core';
import { CommonModule } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatIconModule } from '@angular/material/icon';
import { TranslateModule } from '@ngx-translate/core';

@Component({
  selector: 'app-landing-cta',
  standalone: true,
  imports: [CommonModule, RouterLink, MatIconModule, TranslateModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="wrap" id="inscription">
      <div class="inner">
        <h2>{{ 'landing.cta.title' | translate }}</h2>
        <p class="lead">{{ 'landing.cta.lead' | translate }}</p>
        <div class="ctas">
          <a routerLink="/auth/register" [queryParams]="{ role: 'ELEVEUR' }" class="btn btn--primary">
            <mat-icon>agriculture</mat-icon>
            Je suis éleveur
          </a>
          <a routerLink="/auth/register" [queryParams]="{ role: 'CLIENT' }" class="btn btn--secondary">
            <mat-icon>shopping_cart</mat-icon>
            Je suis client
          </a>
        </div>
        <p class="micro">
          Déjà membre ? <a routerLink="/auth/login">Se connecter</a>
        </p>
      </div>
    </section>
  `,
  styles: [`
    .wrap {
      background: var(--faso-gradient-brand);
      padding: var(--faso-space-16) var(--faso-space-4);
      color: #FFFFFF;
      text-align: center;
    }
    .inner { max-width: 780px; margin-inline: auto; }
    h2 {
      color: inherit;
      font-size: clamp(1.75rem, 4vw, 2.75rem);
      font-weight: var(--faso-weight-bold);
      line-height: 1.2;
      margin: 0 0 var(--faso-space-4);
    }
    .lead {
      opacity: 0.95;
      font-size: var(--faso-text-lg);
      max-width: 52ch;
      margin: 0 auto var(--faso-space-8);
    }
    .ctas {
      display: flex;
      flex-wrap: wrap;
      justify-content: center;
      gap: var(--faso-space-3);
    }
    .btn {
      display: inline-flex;
      align-items: center;
      gap: 8px;
      padding: 14px 28px;
      border-radius: var(--faso-radius-pill);
      font-weight: var(--faso-weight-semibold);
      font-size: var(--faso-text-base);
      text-decoration: none;
      transition: transform var(--faso-duration-fast) var(--faso-ease-standard),
                  box-shadow var(--faso-duration-fast) var(--faso-ease-standard);
    }
    .btn:hover { transform: translateY(-2px); box-shadow: var(--faso-elevation-hover); text-decoration: none; }
    .btn--primary {
      background: #FFFFFF;
      color: var(--faso-primary-700);
    }
    .btn--secondary {
      background: rgba(255,255,255,0.12);
      color: #FFFFFF;
      border: 1.5px solid rgba(255,255,255,0.75);
    }
    .micro {
      margin-top: var(--faso-space-6);
      opacity: 0.92;
      font-size: var(--faso-text-sm);
    }
    .micro a { color: inherit; text-decoration: underline; }

    @media (prefers-reduced-motion: reduce) {
      .btn:hover { transform: none; }
    }
  `],
})
export class LandingCtaComponent {}
