import { ChangeDetectionStrategy, Component } from '@angular/core';
import { CommonModule } from '@angular/common';
import { MatIconModule } from '@angular/material/icon';
import { TranslateModule } from '@ngx-translate/core';

@Component({
  selector: 'app-landing-trust',
  standalone: true,
  imports: [CommonModule, MatIconModule, TranslateModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="trust" id="confiance">
      <div class="inner">
        <article class="pillar">
          <span class="icon"><mat-icon>verified_user</mat-icon></span>
          <h3>{{ 'landing.trust.p1_title' | translate }}</h3>
          <p>{{ 'landing.trust.p1_desc' | translate }}</p>
        </article>
        <article class="pillar">
          <span class="icon"><mat-icon>eco</mat-icon></span>
          <h3>{{ 'landing.trust.p2_title' | translate }}</h3>
          <p>{{ 'landing.trust.p2_desc' | translate }}</p>
        </article>
        <article class="pillar">
          <span class="icon"><mat-icon>local_shipping</mat-icon></span>
          <h3>{{ 'landing.trust.p3_title' | translate }}</h3>
          <p>{{ 'landing.trust.p3_desc' | translate }}</p>
        </article>
      </div>
    </section>
  `,
  styles: [`
    .trust {
      background: var(--faso-surface);
      padding: var(--faso-space-12) var(--faso-space-4);
    }
    .inner {
      max-width: 1200px;
      margin-inline: auto;
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
      gap: var(--faso-space-6);
    }
    .pillar {
      text-align: center;
      padding: var(--faso-space-6) var(--faso-space-4);
    }
    .icon {
      display: inline-flex;
      width: 64px;
      height: 64px;
      border-radius: 50%;
      background: var(--faso-primary-50);
      color: var(--faso-primary-700);
      align-items: center;
      justify-content: center;
      margin-bottom: var(--faso-space-4);
    }
    .icon mat-icon { font-size: 32px; width: 32px; height: 32px; }
    h3 {
      margin: 0 0 var(--faso-space-2);
      font-size: var(--faso-text-xl);
      font-weight: var(--faso-weight-semibold);
      color: var(--faso-text);
    }
    p {
      color: var(--faso-text-muted);
      max-width: 38ch;
      margin: 0 auto;
    }
  `],
})
export class LandingTrustComponent {}
