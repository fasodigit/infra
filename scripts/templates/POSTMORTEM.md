<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- Copyright (C) 2026 FASO DIGITALISATION -->
<!-- Postmortem template — Google-SRE style, rempli auto par postmortem-bot puis complété humain sous 48h. -->

# Postmortem: {{ALERT_NAME}} on {{SERVICE}}

- **Incident ID:** `{{FINGERPRINT}}`
- **Detected (UTC):** {{TIMESTAMP_UTC}}
- **Service:** `{{SERVICE}}`
- **Severity:** `{{SEVERITY}}`
- **Oncall:** {{ASSIGNEE}}
- **Grafana:** {{GRAFANA_URL}}
- **Runbook:** {{RUNBOOK_URL}}

> Remplir toutes les sections dans les **48 heures** suivant la resolution.
> Ne pas chercher de coupable — focus sur le systeme.

---

## 1. Summary

_(2-3 phrases : quoi, qui a ete impacte, combien de temps.)_

`{{SUMMARY}}`

## 2. Impact

- **User-facing impact:** _(ex: 12% des appels /login en erreur)_
- **Duration:** _(detection -> resolution)_
- **Affected services:** _(downstream, upstream)_
- **Data loss?:** _(oui/non, scope)_
- **SLO burn:** _(% error budget consomme)_

## 3. Detection

- **How was it detected?:** _(alerte auto `{{ALERT_NAME}}`, client report, chaos test...)_
- **Detection latency:** _(time entre debut incident et alerte)_
- **Was the alert actionable?:** _(oui/non, runbook suivi ?)_

## 4. Timeline (UTC)

| Time | Event |
|------|-------|
| {{TIMESTAMP_UTC}} | Alert `{{ALERT_NAME}}` firing |
| ... | ... |
| ... | Resolved |

## 5. Root Cause

_(5 Whys ou Ishikawa. Decrire la chaine de causalite technique + organisationnelle.)_

## 6. Resolution & Remediation

_(Actions immediates qui ont stoppe l'incident.)_

- [ ] Mitigation step 1
- [ ] Rollback / hotfix
- [ ] Validation via metriques

## 7. Lessons Learned

### What went well

-

### What went wrong

-

### Where we got lucky

-

## 8. Action Items

_(SMART : owner + deadline + ticket.)_

| # | Action | Type | Owner | Deadline | Ticket |
|---|--------|------|-------|----------|--------|
| 1 | _Corrective ex: fix race condition_ | corrective | @owner | YYYY-MM-DD | #... |
| 2 | _Preventive ex: ajouter chaos test_ | preventive | @owner | YYYY-MM-DD | #... |
| 3 | _Process ex: reviser runbook_ | process | @owner | YYYY-MM-DD | #... |

## 9. Supporting Data

- Dashboards: {{GRAFANA_URL}}
- Runbook used: {{RUNBOOK_URL}}
- Logs query: _(a completer)_
- Traces: _(a completer)_
