# FASO Digitalisation — Frontend Design System

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

Système de design unifié pour toutes les plateformes frontend FASO (Poulets, ÉTAT-CIVIL, HOSPITAL, E-TICKET, VOUCHERS, SOGESY, E-SCHOOL, ALT-MISSION, FASO-KALAN).

## Structure

```
src/styles/
├── _tokens.scss            # design tokens (couleurs, espacement, typo, motion)
├── _mixins.scss            # breakpoints, a11y, responsive
├── _utilities.scss         # classes utilitaires (.faso-*, KPI, skeleton, empty)
├── _material-overrides.scss# thème Angular Material 21 (M3) + palette FASO
└── _components-home.scss   # styles section hero + features + poulet-card
src/styles.scss             # point d'entrée, reset, typography, imports
```

## Palette souveraine

Tokens prefixés `--faso-*`, référence la **Burkina Faso flag** (rouge `#CE1126`, jaune `#FCD116`, vert `#009E49`) + palette tonale Material 3 dérivée du vert agricole.

| Token | Usage |
|-------|-------|
| `--faso-primary-*` | 50→900, vert agriculture (brand) |
| `--faso-accent-*` | 50→900, amber harvest |
| `--faso-success / --faso-warning / --faso-danger / --faso-info` | semantic + `-bg` light variants |
| `--faso-flag-red / -yellow / -green` | bande drapeau hero |
| `--faso-text / -muted / -subtle / -inverse` | texte |
| `--faso-bg / -surface / -surface-alt / -surface-raised` | fonds |
| `--faso-border / -border-strong` | séparateurs |

## Dark mode

Auto via `prefers-color-scheme: dark` + override manuel `<html data-theme="dark">`.
Les tokens se reconfigurent — aucun composant à toucher.

## Accessibilité

- **focus-visible ring** cohérent sur tous les éléments interactifs (mixin `@include focus-ring`)
- Support `prefers-reduced-motion` — toutes les transforms de hover sont désactivées
- Support `prefers-contrast: more` + `forced-colors: active` (Windows HCM)
- Typographie Inter avec fallback system-ui → lisibilité optimale
- Contraste WCAG AA min partout, AAA sur core actions

## Responsivité

Breakpoints : `sm 640 · md 768 · lg 1024 · xl 1280 · 2xl 1440` via mixins `@include bp-md { … }`.
Mobile-first. `env(safe-area-inset-*)` supporté pour iOS/Android PWA.

## Classes utilitaires principales

```html
<div class="faso-container faso-stack">
  <div class="faso-kpi">
    <span class="faso-kpi__label">Commandes aujourd'hui</span>
    <span class="faso-kpi__value">128</span>
    <span class="faso-kpi__delta is-up">+12 %</span>
  </div>
  <span class="faso-badge is-success">Livré</span>
  <div class="faso-skeleton" style="height: 24px;"></div>
</div>
```

## Guide d'adoption (autres plateformes FASO)

1. Copier `src/styles/` vers `src/styles/` du frontend cible.
2. Replacer `src/styles.scss` par la version importée.
3. Adapter les tokens applicatifs (overrides dans `_tokens-app.scss`).
4. Vérifier `ng build --configuration production` — aucune régression attendue (backwards-compat avec `.container`, `.card-grid`, `.page-header`, `.snackbar-*`).

## Inspiration / sources

- Material Design 3 tonal palette — algorithme `@angular/material` v21 `mat.theme()`
- OpenProps + Radix Colors pour l'échelle tonale
- Apple HIG + Atlassian Design System pour motion/a11y
- Drapeau Burkina Faso — identité nationale
