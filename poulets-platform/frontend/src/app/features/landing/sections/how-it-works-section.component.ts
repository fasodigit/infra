import { ChangeDetectionStrategy, Component, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { MatIconModule } from '@angular/material/icon';
import { TranslateModule } from '@ngx-translate/core';
import { SectionHeaderComponent } from '@shared/components/section-header/section-header.component';

type Role = 'client' | 'eleveur';
interface Step { icon: string; title: string; desc: string; }

@Component({
  selector: 'app-landing-how-it-works',
  standalone: true,
  imports: [CommonModule, MatIconModule, TranslateModule, SectionHeaderComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="wrap" id="fonctionnalites">
      <div class="inner">
        <app-section-header
          kicker="En 3 étapes"
          [title]="'landing.how.title' | translate"
          [subtitle]="'landing.how.subtitle' | translate"
        />

        <div class="tabs" role="tablist">
          <button
            type="button"
            role="tab"
            [class.active]="role() === 'client'"
            [attr.aria-selected]="role() === 'client'"
            (click)="role.set('client')"
          >
            <mat-icon>shopping_cart</mat-icon>
            Je suis client
          </button>
          <button
            type="button"
            role="tab"
            [class.active]="role() === 'eleveur'"
            [attr.aria-selected]="role() === 'eleveur'"
            (click)="role.set('eleveur')"
          >
            <mat-icon>agriculture</mat-icon>
            Je suis éleveur
          </button>
        </div>

        <div class="steps" role="tabpanel">
          @for (s of (role() === 'client' ? clientSteps : eleveurSteps); track s.title; let i = $index) {
            <article class="step">
              <span class="num">{{ i + 1 }}</span>
              <span class="step-icon"><mat-icon>{{ s.icon }}</mat-icon></span>
              <h4>{{ s.title }}</h4>
              <p>{{ s.desc }}</p>
            </article>
          }
        </div>
      </div>
    </section>
  `,
  styles: [`
    .wrap {
      background: var(--faso-bg);
      padding: var(--faso-space-12) var(--faso-space-4);
    }
    .inner { max-width: 1100px; margin-inline: auto; }

    .tabs {
      display: inline-flex;
      padding: 4px;
      gap: 4px;
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-pill);
      margin-bottom: var(--faso-space-6);
    }
    .tabs button {
      display: inline-flex;
      align-items: center;
      gap: 6px;
      padding: 8px 20px;
      border: none;
      background: transparent;
      border-radius: var(--faso-radius-pill);
      cursor: pointer;
      color: var(--faso-text-muted);
      font-weight: var(--faso-weight-semibold);
      transition: background var(--faso-duration-fast) var(--faso-ease-standard);
    }
    .tabs button.active {
      background: var(--faso-primary-600);
      color: var(--faso-text-inverse);
    }
    .tabs button mat-icon { font-size: 18px; width: 18px; height: 18px; }

    .steps {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
      gap: var(--faso-space-6);
    }
    .step {
      position: relative;
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
      padding: var(--faso-space-6);
    }
    .num {
      position: absolute;
      top: var(--faso-space-4);
      right: var(--faso-space-4);
      color: var(--faso-accent-600);
      font-size: var(--faso-text-4xl);
      font-weight: var(--faso-weight-bold);
      opacity: 0.25;
      line-height: 1;
    }
    .step-icon {
      display: inline-flex;
      width: 48px; height: 48px;
      border-radius: 12px;
      background: var(--faso-primary-50);
      color: var(--faso-primary-700);
      align-items: center;
      justify-content: center;
      margin-bottom: var(--faso-space-3);
    }
    .step-icon mat-icon { font-size: 24px; width: 24px; height: 24px; }
    h4 { margin: 0 0 var(--faso-space-2); font-size: var(--faso-text-lg); font-weight: var(--faso-weight-semibold); }
    p { margin: 0; color: var(--faso-text-muted); }
  `],
})
export class LandingHowItWorksComponent {
  role = signal<Role>('client');

  readonly clientSteps: Step[] = [
    { icon: 'person_add', title: 'Je crée mon compte client', desc: 'Inscription rapide avec téléphone Burkina. Vérification simple.' },
    { icon: 'search', title: 'Je trouve un éleveur de confiance', desc: 'Recherche par race, région, prix. Avis et certifications visibles.' },
    { icon: 'local_shipping', title: 'Je reçois mes poulets', desc: 'Livraison directe ou retrait sur place. Paiement mobile money.' },
  ];
  readonly eleveurSteps: Step[] = [
    { icon: 'person_add', title: 'Je crée mon profil éleveur', desc: 'Photos ferme, spécialités, certifications halal / vétérinaire.' },
    { icon: 'campaign', title: 'Je publie mes lots', desc: 'Quantité, poids estimé, date, prix. Boost groupement possible.' },
    { icon: 'payments', title: 'Je reçois mes commandes', desc: 'Les clients réservent. Paiement sécurisé à la livraison.' },
  ];
}
