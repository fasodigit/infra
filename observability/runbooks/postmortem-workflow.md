<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- Copyright (C) 2026 FASO DIGITALISATION -->

# Runbook: Postmortem workflow (48h)

> Une GH issue `[POSTMORTEM] ...` vient d'etre creee automatiquement par le
> `faso-postmortem-bot` suite a une alerte `severity: critical` + `team: sre`.
> Ce runbook decrit **comment la completer dans les 48h** apres resolution
> de l'incident.

## Horloge

| T+ | Action | Owner |
|----|--------|-------|
| 0h | Alerte firing -> bot cree l'issue -> oncall PagerDuty'd | automatique |
| 0-Xh | Mitigation live, annotations Grafana, commentaires issue | oncall |
| Xh | Incident resolved -> notifier resolution dans l'issue | oncall |
| +24h | Draft Summary + Impact + Timeline | oncall |
| +48h | Post-incident review meeting -> remplir Root Cause, Lessons, Action Items | oncall + SRE team |
| +7j | Tous les action items ont un ticket GH avec deadline | SRE lead |

## Sections a remplir (template `INFRA/scripts/templates/POSTMORTEM.md`)

1. **Summary** — 2-3 phrases neutres. Pas d'accusation.
2. **Impact** — user-facing %, duree, services aval, data loss, SLO burn.
3. **Detection** — alerte auto ou signal externe ? detection latency ? runbook actionnable ?
4. **Timeline** — horodatage UTC minute par minute. Copier-coller Grafana annotations.
5. **Root Cause** — 5 Whys. Chercher cause systeme, pas humaine.
6. **Resolution & Remediation** — ce qui a stoppe le saignement.
7. **Lessons Learned** — went well / went wrong / got lucky.
8. **Action Items** — SMART, type (corrective/preventive/process), owner, deadline, ticket.
9. **Supporting Data** — dashboards, logs queries, traces IDs.

## Regles culturelles (blameless)

- Focus sur le **systeme**, pas sur une personne.
- Une action item SANS owner ni deadline = action item fictive.
- Partager publiquement dans `#faso-sre` + archive dans `INFRA/docs/postmortems/YYYY/`.
- Si la meme root cause apparait 2 fois en 90j -> escalation SRE lead.

## Automation contrat

Le bot assure :

- Creation issue avec template pre-rempli (labels, fingerprint, oncall assignee).
- Deduplication : si l'alerte reste firing, le bot commente au lieu de creer un doublon.
- `send_resolved: false` — la resolution est ajoutee manuellement par l'oncall.

Le bot **ne ferme jamais** une issue : la cloture est une decision humaine
apres validation de tous les action items.

## Validation d'un postmortem "done"

- [ ] Toutes les sections du template sont remplies (pas de `TODO`).
- [ ] Timeline avec horodatage UTC minute par minute.
- [ ] >= 1 action item corrective + >= 1 preventive.
- [ ] Chaque action item -> ticket GH distinct avec deadline.
- [ ] Review pairwise par un autre SRE.
- [ ] Archive dans `INFRA/docs/postmortems/YYYY/YYYYMMDD-<slug>.md`.
