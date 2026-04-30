<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

# terroir-web-admin

Back-office union/exportateur (React + Vite, port `4810`).

**Statut** : placeholder P0.A. Implémentation en **Phase TERROIR P1** (cf.
`INFRA/terroir/docs/ULTRAPLAN-TERROIR-2026-04-30.md` §6 P1.7).

## Périmètre P1

- Dashboard KYC validation producteurs.
- Carte parcelles interactive (Leaflet + GeoJSON).
- Detail parcelle avec status EUDR + evidence.
- Export DDS preview (PDF).
- Auth Kratos session via ARMAGEDDON :8080.

## Stack cible

- Vite (React 19, TypeScript strict).
- TanStack Query pour fetch ARMAGEDDON.
- MapLibre GL JS pour cartographie.
- shadcn/ui + Tailwind v4.

## Bootstrap (P1)

```bash
cd INFRA/terroir/web-admin
bun create vite . --template react-ts
bun install
bun run dev   # :4810
```

## Port-policy

`terroir-web-admin: 4810` (frontend tier, public). Cf. `INFRA/port-policy.yaml`.
