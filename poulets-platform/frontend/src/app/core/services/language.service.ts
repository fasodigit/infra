// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { Injectable, inject, signal } from '@angular/core';
import { TranslateService } from '@ngx-translate/core';

/**
 * Supported language codes for Poulets BF platform.
 *
 * Fallback chain: mos → fr → en  (see loadTranslation in TranslateModule config)
 */
export type SupportedLang = 'fr' | 'en' | 'mos' | 'dyu' | 'ful';

export const SUPPORTED_LANGS: SupportedLang[] = ['fr', 'en', 'mos', 'dyu', 'ful'];

/**
 * Geographic coordinates → default language heuristic.
 * Burkina Faso bounding box: lat 9.4–15.1, lon -5.5–2.4
 * This is a first-pass approximation; proper per-region mapping
 * requires validation by Académie nationale des langues BF.
 *
 * TODO: replace with confirmed per-region data from INSS Ouaga.
 */
const GPS_REGION_LANG: Array<{ minLat: number; maxLat: number; minLon: number; maxLon: number; lang: SupportedLang }> = [
  // Centre/Plateau-Central → Mooré dominant
  { minLat: 11.8, maxLat: 13.0, minLon: -1.5, maxLon: 1.2, lang: 'mos' },
  // Hauts-Bassins / Cascades → Dioula dominant
  { minLat: 10.2, maxLat: 12.0, minLon: -5.5, maxLon: -2.0, lang: 'dyu' },
  // Sahel / Est → Fulfulde dominant
  { minLat: 12.5, maxLat: 15.1, minLon: 0.0, maxLon: 2.4, lang: 'ful' },
];

const STORAGE_KEY = 'faso_lang';

@Injectable({ providedIn: 'root' })
export class LanguageService {
  private readonly translate = inject(TranslateService);

  /** Currently active language, reactive signal. */
  readonly currentLang = signal<SupportedLang>(this.resolveInitialLang());

  constructor() {
    this.applyLang(this.currentLang());
  }

  /**
   * Switch language at runtime, persist to localStorage.
   */
  use(lang: SupportedLang): void {
    this.translate.use(lang);
    this.currentLang.set(lang);
    this.persist(lang);
  }

  /**
   * Attempt GPS-based language detection.
   * Falls back to browser language if geolocation fails or is denied.
   * Returns a Promise that resolves once the detected lang is applied.
   */
  detectFromGps(): Promise<SupportedLang> {
    if (!('geolocation' in navigator)) {
      return Promise.resolve(this.currentLang());
    }

    return new Promise((resolve) => {
      navigator.geolocation.getCurrentPosition(
        (pos) => {
          const lang = this.gpsToLang(pos.coords.latitude, pos.coords.longitude);
          if (lang) {
            this.use(lang);
            resolve(lang);
          } else {
            resolve(this.currentLang());
          }
        },
        () => {
          // Permission denied or error — keep current
          resolve(this.currentLang());
        },
        { timeout: 5000, enableHighAccuracy: false }
      );
    });
  }

  // ────────────────────────────────────────────────────────────
  // Private helpers
  // ────────────────────────────────────────────────────────────

  private resolveInitialLang(): SupportedLang {
    // 1. Persisted preference
    const stored = this.readPersisted();
    if (stored) return stored;

    // 2. Browser language
    const browser = this.parseBrowserLang();
    if (browser) return browser;

    // 3. Default
    return 'fr';
  }

  private applyLang(lang: SupportedLang): void {
    this.translate.addLangs(SUPPORTED_LANGS);
    this.translate.setDefaultLang('fr');
    // Hierarchical fallback: mos/dyu/ful → fr → en
    // ngx-translate uses setDefaultLang as top-level fallback.
    // For native langs, the JSON files include only partial keys;
    // missing keys automatically resolve from 'fr' (default).
    this.translate.use(lang);
  }

  private gpsToLang(lat: number, lon: number): SupportedLang | null {
    for (const region of GPS_REGION_LANG) {
      if (lat >= region.minLat && lat <= region.maxLat && lon >= region.minLon && lon <= region.maxLon) {
        return region.lang;
      }
    }
    return null;
  }

  private parseBrowserLang(): SupportedLang | null {
    const raw = navigator.language?.toLowerCase() ?? '';
    if (raw.startsWith('fr')) return 'fr';
    if (raw.startsWith('en')) return 'en';
    if (raw === 'mos' || raw.startsWith('mos-')) return 'mos';
    if (raw === 'dyu' || raw.startsWith('dyu-')) return 'dyu';
    if (raw === 'ful' || raw.startsWith('ful-') || raw.startsWith('ff')) return 'ful';
    return null;
  }

  private persist(lang: SupportedLang): void {
    try {
      localStorage.setItem(STORAGE_KEY, lang);
    } catch {
      // Storage unavailable (private browsing, storage quota, etc.)
    }
  }

  private readPersisted(): SupportedLang | null {
    try {
      const val = localStorage.getItem(STORAGE_KEY);
      if (val && (SUPPORTED_LANGS as string[]).includes(val)) {
        return val as SupportedLang;
      }
    } catch {
      // Storage unavailable
    }
    return null;
  }
}
