// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { Component, inject, ChangeDetectionStrategy, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { MatButtonModule } from '@angular/material/button';
import { MatMenuModule } from '@angular/material/menu';
import { MatIconModule } from '@angular/material/icon';
import { MatTooltipModule } from '@angular/material/tooltip';
import { TranslateModule, TranslateService } from '@ngx-translate/core';

interface LangOption {
  code: 'fr' | 'mos' | 'dyu' | 'en';
  label: string;
  flag: string;
}

/**
 * Enriched language switcher for Poulets BF.
 *
 * Supports the 4 pilot languages prioritised for Burkina Faso:
 *   - FR  (French, official)
 *   - MOS (Mooré, dominant center)
 *   - DYU (Dioula, dominant west/south-west)
 *   - EN  (English, international fallback)
 *
 * Choice is persisted to `localStorage` under key `i18n.lang` AND under the
 * legacy `faso_lang` key (used by `LanguageService`) to preserve backward
 * compatibility.
 */
const LANGUAGES: LangOption[] = [
  { code: 'fr',  label: 'common.french',  flag: 'FR' },
  { code: 'mos', label: 'common.moore',   flag: 'MOS' },
  { code: 'dyu', label: 'common.dioula',  flag: 'DYU' },
  { code: 'en',  label: 'common.english', flag: 'EN' },
];

const STORAGE_KEY_NEW = 'i18n.lang';
const STORAGE_KEY_LEGACY = 'faso_lang';

@Component({
  selector: 'app-language-switcher',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [
    CommonModule,
    MatButtonModule,
    MatMenuModule,
    MatIconModule,
    MatTooltipModule,
    TranslateModule,
  ],
  template: `
    <button
      mat-icon-button
      [matMenuTriggerFor]="langMenu"
      [matTooltip]="'common.language' | translate"
      [attr.aria-label]="'common.language' | translate"
      data-testid="lang-switcher-trigger"
    >
      <mat-icon>language</mat-icon>
    </button>
    <mat-menu #langMenu="matMenu">
      @for (lang of languages; track lang.code) {
        <button
          mat-menu-item
          (click)="switchLang(lang.code)"
          [class.active-lang]="currentLang() === lang.code"
          [attr.data-testid]="'lang-option-' + lang.code"
        >
          <span class="lang-flag">{{ lang.flag }}</span>
          <span>{{ lang.label | translate }}</span>
        </button>
      }
    </mat-menu>
  `,
  styles: [`
    .lang-flag {
      display: inline-block;
      width: 36px;
      font-weight: 700;
      font-size: 0.8rem;
      color: #666;
    }

    .active-lang {
      background-color: rgba(46, 125, 50, 0.08);
      font-weight: 500;
    }
  `],
})
export class LanguageSwitcherComponent {
  private readonly translate = inject(TranslateService);

  readonly languages = LANGUAGES;
  readonly currentLang = signal(this.resolveInitial());

  switchLang(lang: LangOption['code']): void {
    this.translate.use(lang);
    this.currentLang.set(lang);
    try {
      localStorage.setItem(STORAGE_KEY_NEW, lang);
      localStorage.setItem(STORAGE_KEY_LEGACY, lang);
    } catch {
      // Storage unavailable
    }
  }

  private resolveInitial(): LangOption['code'] {
    try {
      const persisted =
        localStorage.getItem(STORAGE_KEY_NEW) ??
        localStorage.getItem(STORAGE_KEY_LEGACY);
      if (persisted === 'fr' || persisted === 'mos' || persisted === 'dyu' || persisted === 'en') {
        return persisted;
      }
    } catch {
      // Storage unavailable
    }
    const current = this.translate.currentLang || this.translate.defaultLang || 'fr';
    if (current === 'mos' || current === 'dyu' || current === 'en') return current;
    return 'fr';
  }
}
