import { ChangeDetectionStrategy, Component } from '@angular/core';
import { CommonModule } from '@angular/common';
import { TranslateModule } from '@ngx-translate/core';
import { SectionHeaderComponent } from '@shared/components/section-header/section-header.component';
import { CategoryTileComponent } from '@shared/components/category-tile/category-tile.component';

interface Category {
  icon: string;
  label: string;
  hint: string;
  queryParams: Record<string, string>;
}

@Component({
  selector: 'app-landing-categories',
  standalone: true,
  imports: [CommonModule, TranslateModule, SectionHeaderComponent, CategoryTileComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="wrap" id="categories">
      <div class="inner">
        <app-section-header
          kicker="Parcourir"
          [title]="'landing.categories.title' | translate"
          [subtitle]="'landing.categories.subtitle' | translate"
          [linkLabel]="'landing.categories.all' | translate"
          linkTo="/marketplace/annonces"
        />
        <div class="grid">
          @for (c of categories; track c.label) {
            <app-category-tile
              [icon]="c.icon"
              [label]="c.label"
              [hint]="c.hint"
              routerLink="/marketplace/annonces"
              [queryParams]="c.queryParams"
            />
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
    .inner {
      max-width: 1200px;
      margin-inline: auto;
    }
    .grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
      gap: var(--faso-space-4);
    }
  `],
})
export class LandingCategoriesComponent {
  readonly categories: Category[] = [
    { icon: 'pets',           label: 'Race locale',     hint: 'Bikié, Poulet bicyclette', queryParams: { race: 'LOCALE' } },
    { icon: 'egg',             label: 'Pondeuses',       hint: 'Œufs de ferme',           queryParams: { type: 'PONDEUSE' } },
    { icon: 'restaurant',      label: 'Poulets de chair',hint: '45-60 jours',             queryParams: { type: 'CHAIR' } },
    { icon: 'eco',             label: 'Bio',             hint: 'Nourriture naturelle',    queryParams: { bio: 'true' } },
    { icon: 'verified',        label: 'Halal certifié',  hint: 'Abattage tracé',          queryParams: { halal: 'true' } },
    { icon: 'groups',          label: 'Groupement',      hint: 'Coopératives',            queryParams: { groupement: 'true' } },
  ];
}
