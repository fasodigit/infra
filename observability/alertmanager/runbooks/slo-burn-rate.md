<!--
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (C) 2026 FASO DIGITALISATION
-->

# Runbook — SLO multi-window multi-burn-rate alerts

Ce runbook couvre les alertes générées par Sloth (fichiers `INFRA/observability/slo/*.slo.yaml`)
en suivant le modèle **multi-window multi-burn-rate** décrit au chapitre 4 du
*Google SRE Workbook* (« Alerting on SLOs »).

## 1. Rappel du modèle

Un *burn rate* (BR) est le ratio de consommation de l'error budget par rapport
à la vitesse nominale. BR=1 consomme tout le budget exactement sur la période
(30 jours par défaut). BR=14.4 consomme 2% du budget en 1 heure.

Sloth génère deux tiers d'alertes par défaut :

| Tier      | Fenêtres observées     | Budget consommé | Gravité   | Action                       |
|-----------|------------------------|-----------------|-----------|------------------------------|
| **page**  | 1h + 5m (rapide)       | **2% en 1h**    | critical  | page on-call immédiate       |
| **page**  | 6h + 30m (moyen)       | **5% en 6h**    | critical  | page on-call                 |
| **ticket**| 24h + 2h (lent)        | **10% en 24h**  | warning   | ticket Jira — fix sous 24h   |
| **ticket**| 3j + 6h (très lent)    | **10% en 3j**   | warning   | ticket Jira — fix sous 72h   |

La condition *multi-window* (burn rate court **ET** burn rate long) réduit les
faux positifs d'un spike bref et les faux négatifs d'une dérive lente.

## 2. Règles de paging

### 2a. 2% budget consommé en 1 heure → `severity: critical` (page)

Recording rule Sloth associée :

```
slo:sli_error:ratio_rate1h > (14.4 * objective_ratio)
AND
slo:sli_error:ratio_rate5m > (14.4 * objective_ratio)
```

**Conséquence** : Alertmanager route vers PagerDuty / OpsGenie FASO — SLA
de réponse < 5 min.

### 2b. 10% budget consommé en 6 heures → `severity: warning` (ticket)

```
slo:sli_error:ratio_rate6h > (6 * objective_ratio)
AND
slo:sli_error:ratio_rate30m > (6 * objective_ratio)
```

**Conséquence** : ticket Jira créé automatiquement, assigné à l'équipe owner
(label `team:` du SLO).

## 3. Checklist de réponse (on-call)

1. **Identifier le SLO brûlant** :
   - Ouvrir le dashboard Grafana `FASO — SLO Overview (Sloth)` (UID `faso-slo-overview`).
   - Filtrer sur `sloth_service` (variable `$service`).
   - Noter `sloth_slo` et la fenêtre déclenchante (1h / 6h / 24h / 3j).
2. **Corréler** :
   - Logs Loki : `{service="<svc>"} |= "error"` sur la fenêtre.
   - Traces Tempo : top endpoints lents.
   - Métriques RED (rate/errors/duration) par endpoint.
3. **Classifier** :
   - Incident *brownout* (burn 1h élevé, 6h faible) → mitigation rapide (rollback, circuit breaker, rate limit).
   - Dérive lente (burn 24h/3j) → planifier fix sprint en cours.
4. **Mitiger** :
   - Rollback du dernier déploiement si corrélation temporelle.
   - Scaling horizontal si saturation ressources.
   - Dégradation ciblée (feature flag GrowthBook) si endpoint identifié.
5. **Documenter** :
   - Post-mortem si page critical (cf. `INFRA/observability/alertmanager/POSTMORTEM-README.md`).
   - Mettre à jour ce runbook si nouveau pattern d'incident.

## 4. Silences & maintenance

- Fenêtre de maintenance planifiée → créer un silence Alertmanager ciblé
  sur `sloth_service=<svc>` via `amtool silence add`.
- Ne **jamais** désactiver un SLO en production — si objectif irréaliste,
  ouvrir une PR sur `INFRA/observability/slo/<svc>.slo.yaml` pour ajuster
  l'objectif (revue SRE obligatoire).

## 5. Références

- Google SRE Workbook, ch. 4 — *Alerting on SLOs* : https://sre.google/workbook/alerting-on-slos/
- Sloth docs : https://sloth.dev/
- FASO observability stack : `INFRA/observability/grafana/podman-compose.observability.yml`
- Génération locale des rules : `bash INFRA/observability/slo/generate-rules.sh`
