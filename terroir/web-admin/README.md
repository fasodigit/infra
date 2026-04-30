<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

# terroir-web-admin

Back-office union/exportateur (React 19 + Vite 6 + TypeScript strict, port `4810`).

**Statut** : MVP P1.F implémenté (structure complète, install Bun différée à
P1.H cycle-fix). Cf. `INFRA/terroir/docs/ULTRAPLAN-TERROIR-2026-04-30.md` §6 P1.7.

## Périmètre P1

- Login Kratos (proxy ARMAGEDDON `:8080/auth/*`).
- Dashboard KPIs union (producteurs / parcelles / DDS / alertes EUDR).
- Liste producteurs paginée + recherche + filtre coopérative + KYC.
- Détail producteur : profil + parcelles + actions (approve KYC / suspend / reset MFA).
- Carte parcelles Leaflet + GeoJSON layer (couleur EUDR status).
- Détail parcelle : carte + EUDR validation + DDS PDF preview + bouton TRACES NT.
- Journal d'audit timeline filtrable (date / actor / action) + lien Jaeger.

## Stack

- React 19 + Vite 6 + TypeScript strict.
- TanStack Query v5 pour fetch ARMAGEDDON.
- React Router v7.
- Leaflet 1.9 + react-leaflet (tiles OSM CC BY-SA, souverain via Nominatim mirror si dispo).
- i18next FR/EN (`src/i18n/`).
- Pas de Firebase, pas de Google Maps, pas d'AWS Amplify (souveraineté).

## Bootstrap (P1.H — cycle-fix)

```bash
cd INFRA/terroir/web-admin
bun install
bun run typecheck
bun run dev   # :4810
```

Pré-requis : ARMAGEDDON `:8080` UP (CORS autorise `:4810`), terroir-core `:8830`
joignable via route `/api/terroir/core/*`, Kratos `:4433` derrière `/auth/*`.

## Port-policy

`terroir-web-admin: 4810` (frontend tier, public). Cf. `INFRA/port-policy.yaml`.

## Auth flow

1. `GET /auth/self-service/login/browser` (Kratos via ARMAGEDDON) → flow ID.
2. `POST <flow.action>` avec `identifier=email` + `password` → cookie `ory_kratos_session`.
3. `GET /auth/whoami` pour récupérer la session sur chaque protected route.
4. `useAuth()` + `<RequireAuth>` redirigent vers `/login` si 401.

## Citations légales (footer obligatoire)

- Hansen Global Forest Change v1.11 — CC BY 4.0
- JRC EU forest map 2020 — CC BY 4.0
- OpenStreetMap contributors — CC BY-SA

Cf. `INFRA/terroir/docs/LICENSES-GEO.md` (P1.C).

## Tests E2E

À écrire en P1.G (Playwright). Specs ciblent :
- `/login` (happy + 401)
- `/dashboard` (KPIs render)
- `/producers` (search + pagination + click → detail)
- `/parcels` (carte render + filtre EUDR)
- `/parcel/:id` (DDS submit TRACES NT)

Acteurs SUPER-ADMIN seedés (Aminata, Souleymane) cf. `tests-e2e/fixtures/actors.ts`.
