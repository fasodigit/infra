<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

# terroir-buyer-portal

Portail acheteurs/exportateurs invitation-only (Next.js 16, port `4811`).

**Statut** : placeholder P0.A. Implémentation en **Phase TERROIR P3** (cf.
`INFRA/terroir/docs/ULTRAPLAN-TERROIR-2026-04-30.md` §8 P3.3).

## Périmètre P3

- Public listings catalog (SEO-friendly).
- Acheteur signup invitation-only (token email signé exportateur).
- Détail lot + contract signature workflow (Vault PKI).
- DDS download (PDF signé, JWT timestampé pour non-repudiation).

## Stack cible

- Next.js 16 (App Router, RSC).
- Auth : token JWT signé par exportateur (scope `buyer.<exporter_id>`).
- Cookie store via `next/headers`.
- Tailwind v4 + shadcn/ui.

## Bootstrap (P3)

```bash
cd INFRA/terroir/buyer-portal
bunx create-next-app@latest . --typescript --app --tailwind
bun install
bun run dev   # :4811
```

## Port-policy

`terroir-buyer-portal: 4811` (frontend tier, public). Cf. `INFRA/port-policy.yaml`.
