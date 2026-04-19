# Internationalisation — Poulets BF Platform

Poulets BF supporte 5 langues : **Français (fr)**, **Anglais (en)**,
**Mooré (mos)**, **Dioula (dyu)** et **Fulfulde (ful)**.

---

## Architecture

| Fichier | Rôle |
|---------|------|
| `src/assets/i18n/fr.json` | Source de vérité — toutes les clés en français |
| `src/assets/i18n/en.json` | Traduction anglaise complète |
| `src/assets/i18n/mos.json` | Mooré — clés pilotes traduites, reste en placeholder |
| `src/assets/i18n/dyu.json` | Dioula — clés pilotes traduites, reste en placeholder |
| `src/assets/i18n/ful.json` | Fulfulde — placeholders, TODO traducteurs natifs |
| `src/app/core/services/language.service.ts` | Détection browser + GPS fallback, switch runtime, localStorage |
| `src/app/shared/components/language-switcher/language-switcher.component.ts` | Sélecteur 5 langues avec drapeaux |

**Chaîne de fallback** : `mos/dyu/ful → fr → en`
Clés manquantes dans une langue native remontent automatiquement vers `fr` (via `setDefaultLang`).

---

## Clés pilotes implémentées

Les clés suivantes sont les premières à être traduites en Mooré et Dioula :

| Clé | FR | MOS | DYU |
|-----|----|-----|-----|
| `COMMON.LOGIN` | Connexion | Loog-y tõnd | Sεbε don |
| `COMMON.LOGOUT` | Déconnexion | Yit tõnd | Bɔ |
| `COMMON.SUBMIT` | Envoyer | Kõnd | Ci lajɛ |
| `COMMON.CANCEL` | Annuler | Bas | A to |
| `AUTH.SIGNIN` | Se connecter | Loog-y tõnd | Sεbε don |
| `AUTH.LOGOUT` | Déconnexion | Yit tõnd | Bɔ |

> **Note traducteurs** : Les valeurs préfixées `[MOS]`, `[DYU]`, `[FUL]`
> sont des placeholders temporaires qui affichent la valeur française.
> Ils doivent être remplacés par des traductions natives.

---

## Ajouter ou modifier une traduction

### 1. Exporter pour les traducteurs

```bash
cd frontend
./scripts/sync-i18n.sh export mos
# → génère i18n-export-mos.tsv
```

Envoyer le fichier TSV aux traducteurs. Format :
```
KEY    FR_VALUE    MOS_VALUE
COMMON.LOGIN    Connexion    [à traduire]
```

### 2. Réimporter le travail des traducteurs

```bash
./scripts/sync-i18n.sh import mos i18n-export-mos-translated.tsv
```

### 3. Vérifier la couverture

```bash
./scripts/sync-i18n.sh audit
# Output:
# [en]  342/342 keys translated (100%) — OK
# [mos]  28/342 keys translated (8%) — WARN (< 80%)
# [dyu]  26/342 keys translated (7%) — WARN (< 80%)
# [ful]   8/342 keys translated (2%) — WARN (< 80%)
```

### 4. Ajouter une nouvelle clé

1. Ajouter la clé dans `fr.json` (source de vérité)
2. Ajouter la traduction dans `en.json`
3. Ajouter un placeholder `[MOS] valeur_fr` dans `mos.json`, `dyu.json`, `ful.json`
4. Notifier les traducteurs via le processus export/import

---

## CI — Vérification automatique

Le workflow `.github/workflows/i18n-lint.yml` s'exécute à chaque PR touchant `src/assets/i18n/`.

- **Erreur bloquante** : fichier de langue manquant ou JSON invalide
- **Avertissement** : couverture < 80% (ne bloque PAS le build)

---

## Contacts traducteurs

### Institutions de référence

| Institution | Rôle | Contact |
|-------------|------|---------|
| **INSS** — Institut National des Sciences des Sociétés | Recherche linguistique BF | Université Joseph Ki-Zerbo, Ouagadougou |
| **ANLB** — Académie nationale des langues du Burkina Faso | Normalisation orthographique | Ministère de la Culture, Ouagadougou |
| **DGESS** — Direction de l'Éducation bilingue | Pédagogie langues nationales | Ministère de l'Éducation nationale |

### Contact projet

**FASO DIGITALISATION**
Email : fasodigitalisation@gmail.com
Référence : "Traduction Poulets BF — [langue]"

### Budget traducteurs

Le budget pour les traductions nationales est documenté dans le plan projet FASO DIGITALISATION.
Modèle de rémunération suggéré :

- Traduction initiale (par langue) : à négocier avec ANLB
- Relecture native : 2 relecteurs minimum par langue
- Maintenance (nouvelles clés) : contrat cadre annuel recommandé

---

## Ajouter une nouvelle langue

1. Créer `src/assets/i18n/<code>.json` (structure identique à `fr.json`)
2. Ajouter `<code>` à `SUPPORTED_LANGS` dans `language.service.ts`
3. Ajouter l'option dans `language-switcher.component.ts` (tableau `LANGUAGES`)
4. Ajouter la règle GPS dans `language.service.ts` si pertinent géographiquement
5. Mettre à jour ce README

---

## Ressources linguistiques

- Mooré (mos) : dialecte central du mossi — principale langue du Plateau-Central
- Dioula (dyu) : lingua franca du commerce ouest-africain — dominant Hauts-Bassins
- Fulfulde (ful) : langue des Peuls — dominant Sahel et Est

Références orthographiques : alphabet officiel défini par le Décret n°85-404/CNR/PRES
du Burkina Faso et les travaux de l'ANLB.
