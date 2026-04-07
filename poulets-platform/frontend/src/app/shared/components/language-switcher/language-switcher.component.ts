import { Component, inject, ChangeDetectionStrategy, signal } from '@angular/core';
import { CommonModule } from '@angular/common';
import { MatButtonModule } from '@angular/material/button';
import { MatMenuModule } from '@angular/material/menu';
import { MatIconModule } from '@angular/material/icon';
import { MatTooltipModule } from '@angular/material/tooltip';
import { TranslateModule, TranslateService } from '@ngx-translate/core';

interface LangOption {
  code: string;
  label: string;
  flag: string;
}

const LANGUAGES: LangOption[] = [
  { code: 'fr', label: 'common.french', flag: 'FR' },
  { code: 'en', label: 'common.english', flag: 'EN' },
];

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
    >
      <mat-icon>language</mat-icon>
    </button>
    <mat-menu #langMenu="matMenu">
      @for (lang of languages; track lang.code) {
        <button
          mat-menu-item
          (click)="switchLang(lang.code)"
          [class.active-lang]="currentLang() === lang.code"
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
      width: 28px;
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
  readonly currentLang = signal(this.translate.currentLang || this.translate.defaultLang || 'fr');

  switchLang(lang: string): void {
    this.translate.use(lang);
    this.currentLang.set(lang);
    try {
      localStorage.setItem('faso_lang', lang);
    } catch {
      // Storage unavailable
    }
  }
}
