import { ChangeDetectionStrategy, Component, EventEmitter, Input, Output } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';

export interface SearchQuery {
  race?: string;
  location?: string;
  date?: string;
}

@Component({
  selector: 'app-search-hero',
  standalone: true,
  imports: [CommonModule, FormsModule, MatIconModule, MatButtonModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <form class="search" [class.compact]="compact" (submit)="submit($event)" role="search">
      <label class="field field--race">
        <span class="lbl">Race</span>
        <select [(ngModel)]="query.race" name="race">
          <option value="">Toutes races</option>
          @for (r of races; track r) { <option [value]="r">{{ r }}</option> }
        </select>
      </label>

      <span class="sep" aria-hidden="true"></span>

      <label class="field field--loc">
        <span class="lbl">Région</span>
        <input
          type="text"
          [(ngModel)]="query.location"
          name="location"
          placeholder="Ouagadougou, Bobo…"
        >
      </label>

      <span class="sep" aria-hidden="true"></span>

      <label class="field field--date">
        <span class="lbl">Date souhaitée</span>
        <input type="date" [(ngModel)]="query.date" name="date">
      </label>

      <button
        type="submit"
        class="cta"
        [attr.aria-label]="'Lancer la recherche'"
      >
        <mat-icon>search</mat-icon>
        <span>Rechercher</span>
      </button>
    </form>
  `,
  styles: [`
    :host { display: block; width: 100%; }
    .search {
      display: grid;
      grid-template-columns: 1fr auto 1fr auto 1fr auto;
      align-items: stretch;
      padding: var(--faso-space-2);
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-pill);
      box-shadow: var(--faso-elevation-hover);
      max-width: 960px;
      margin-inline: auto;
    }
    .search.compact {
      box-shadow: var(--faso-elevation-card);
    }
    .field {
      display: flex;
      flex-direction: column;
      justify-content: center;
      padding: 6px 20px;
      border-radius: var(--faso-radius-pill);
      cursor: pointer;
      transition: background var(--faso-duration-fast) var(--faso-ease-standard);
    }
    .field:hover { background: var(--faso-surface-alt); }
    .lbl {
      font-size: var(--faso-text-xs);
      font-weight: var(--faso-weight-semibold);
      color: var(--faso-text);
      letter-spacing: 0.02em;
    }
    .field input, .field select {
      border: none;
      background: transparent;
      font-size: var(--faso-text-sm);
      color: var(--faso-text-muted);
      padding: 2px 0;
      font-family: inherit;
      outline: none;
      width: 100%;
    }
    .field input::placeholder { color: var(--faso-text-subtle); }

    .sep {
      width: 1px;
      background: var(--faso-border);
      margin: 10px 0;
    }

    .cta {
      display: inline-flex;
      align-items: center;
      gap: 6px;
      margin-left: var(--faso-space-2);
      padding: 0 var(--faso-space-5);
      background: var(--faso-primary-600);
      color: var(--faso-text-inverse);
      border: none;
      border-radius: var(--faso-radius-pill);
      font-weight: var(--faso-weight-semibold);
      cursor: pointer;
      transition: background var(--faso-duration-fast) var(--faso-ease-standard),
                  transform var(--faso-duration-fast) var(--faso-ease-standard);
    }
    .cta:hover { background: var(--faso-primary-700); }
    .cta:active { transform: scale(0.98); }
    .cta mat-icon { font-size: 20px; width: 20px; height: 20px; }

    @media (max-width: 767px) {
      .search {
        grid-template-columns: 1fr;
        border-radius: var(--faso-radius-xl);
        padding: var(--faso-space-3);
        gap: var(--faso-space-2);
      }
      .sep { display: none; }
      .field { padding: var(--faso-space-2) var(--faso-space-3); }
      .cta { width: 100%; justify-content: center; padding: var(--faso-space-3) 0; }
    }
  `],
})
export class SearchHeroComponent {
  @Input() compact = false;
  @Input() races: string[] = [];
  @Input() query: SearchQuery = {};
  @Output() search = new EventEmitter<SearchQuery>();

  submit(ev: Event) {
    ev.preventDefault();
    this.search.emit({ ...this.query });
  }
}
