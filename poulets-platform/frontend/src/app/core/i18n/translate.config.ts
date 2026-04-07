import { Provider } from '@angular/core';
import { provideTranslateHttpLoader } from '@ngx-translate/http-loader';

/**
 * Provides the ngx-translate HTTP loader configured for FASO i18n.
 */
export function provideI18nHttpLoader(): Provider[] {
  return provideTranslateHttpLoader({
    prefix: './assets/i18n/',
    suffix: '.json',
  });
}
