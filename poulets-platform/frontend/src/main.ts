// OTel browser instrumentation — MUST run before Angular bootstrap so the
// WebTracerProvider is the global tracer when components emit spans.
import './instrumentation';

import { bootstrapApplication } from '@angular/platform-browser';
import { provideRouter, withComponentInputBinding } from '@angular/router';
import { provideAnimationsAsync } from '@angular/platform-browser/animations/async';
import { provideHttpClient, withInterceptors, withFetch } from '@angular/common/http';
import { inject } from '@angular/core';
import { provideApollo } from 'apollo-angular';
import { HttpLink } from 'apollo-angular/http';
import { InMemoryCache } from '@apollo/client';
import { provideTranslateService } from '@ngx-translate/core';
import { provideTranslateHttpLoader } from '@ngx-translate/http-loader';

import { AppComponent } from './app/app.component';
import { routes } from './app/app.routes';
import { authInterceptor } from './app/core/interceptors/auth.interceptor';
import { stepUpInterceptor } from './app/core/interceptors/step-up.interceptor';
import { environment } from './environments/environment';
import { PROJECT_CONFIG } from './app/core/config/project-config.token';
import { POULETS_PROJECT_CONFIG } from './app/core/config/poulets-project.config';

bootstrapApplication(AppComponent, {
  providers: [
    { provide: PROJECT_CONFIG, useValue: POULETS_PROJECT_CONFIG },
    provideRouter(routes, withComponentInputBinding()),
    provideAnimationsAsync(),
    provideHttpClient(
      withFetch(),
      withInterceptors([authInterceptor, stepUpInterceptor]),
    ),
    // i18n: ngx-translate
    provideTranslateService({
      defaultLanguage: 'fr',
      fallbackLang: 'fr',
    }),
    provideTranslateHttpLoader({
      prefix: './assets/i18n/',
      suffix: '.json',
    }),
    // Apollo GraphQL
    provideApollo(() => {
      const httpLink = inject(HttpLink);
      return {
        link: httpLink.create({
          uri: `${environment.bffUrl}/api/graphql`,
          withCredentials: true,
        }),
        cache: new InMemoryCache({
          typePolicies: {
            Query: {
              fields: {
                poulets: {
                  merge(existing: unknown, incoming: unknown) {
                    return incoming;
                  },
                },
              },
            },
          },
        }),
        defaultOptions: {
          watchQuery: {
            fetchPolicy: 'cache-and-network',
            errorPolicy: 'all',
          },
        },
      };
    }),
  ],
}).catch((err) => console.error('Application bootstrap failed:', err));
