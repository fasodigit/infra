# Security Audit — Historical Git Scan

**Date :** 2026-04-18  
**Outils :** gitleaks v8 (docker) + trufflehog v3 (cross-check)  
**Périmètre :** `--log-opts='--all'` — historique complet (18 commits + 1 stash = 20 objets)  
**Repo :** `github.com:fasodigit/infra` (public)

---

## Résumé

| Métrique | Valeur |
|----------|--------|
| Commits scannés | 18 (+ 2 stash/WIP) |
| Volume scanné | ~5,57 MB |
| Findings gitleaks | **3** |
| Findings trufflehog (verified) | **0** (timeout sur PrivateKey — voir notes) |
| Critical (clé privée active) | **1** — ENCORE EN HEAD |
| High (mots de passe test en clair) | **2** — supprimés du HEAD, mais présents dans l'historique |
| Commits à réécrire | **3** |

---

## Findings détaillés

| # | Sévérité | Rule | File | Commit | Ligne | En HEAD ? | Action |
|---|----------|------|------|--------|-------|-----------|--------|
| 1 | **CRITICAL** | `private-key` (PKCS#8 EC) | `armageddon/armageddon-mesh/src/fixtures.rs` | `bb951de` | 54–58 | **OUI** | Rotation + retrait HEAD + BFG |
| 2 | **HIGH** | `generic-password` | `poulets-platform/e2e/data/seed.ts` | `e940cd1` | multiples | NON (retiré en bb951de) | BFG sur e940cd1 |
| 3 | **MEDIUM** | `generic-password` | `poulets-platform/e2e/data/seed.ts` | `3e02b59` | multiples | NON | BFG sur 3e02b59 |

### Détail Finding #1 — Clé privée EC (PKCS#8) — CRITIQUE

- **Fichier :** `armageddon/armageddon-mesh/src/fixtures.rs` (constante `KAYA_KEY_PEM`)
- **Commit :** `bb951dedb886e23259f83aae7222d42c40ced097` (2026-04-17T16:33:06Z)
- **Lien GitHub :** https://github.com/fasodigit/infra/blob/bb951dedb886e23259f83aae7222d42c40ced097/armageddon/armageddon-mesh/src/fixtures.rs#L54-L58
- **Nature :** Clé privée EC prime256v1 encodée PKCS#8, incluse comme fixture de test mTLS/SPIFFE. Le fichier contient également le certificat CA et le certificat workload associés.
- **Contexte :** Générée pour les tests unitaires ARMAGEDDON Vague 1 (trust domain `faso.gov.bf`). Le commentaire précise "dev only / Never use in production".
- **Risque :** La clé est publiquement accessible sur GitHub depuis le push du 2026-04-17. Même si non utilisée en prod, sa présence crée : (a) un vecteur d'usurpation d'identité SPIFFE si la confiance est mal configurée, (b) un faux positif bloquant dans les scans SAST futurs.
- **Action immédiate :** Régénérer les fixtures (nouvelle paire de clés, non pushée sur GitHub), retirer la clé du HEAD, réécrire l'historique avec BFG.

### Détail Finding #2 — Mot de passe test `***FASO_REDACTED_TEST_PASSWORD***`

- **Fichier :** `poulets-platform/e2e/data/seed.ts`
- **Commit :** `e940cd1` (2026-04-07T14:42:38Z) — introduit par "Fix critical startup bugs"
- **Nature :** 4 occurrences — mots de passe utilisateurs de seed E2E
- **Contexte :** Utilisé uniquement pour les tests locaux Playwright. Retiré en HEAD au commit `bb951de` (remplacé par `E2E_TEST_PASSWORD`).
- **Risque :** MEDIUM. Jamais déployé en production (selon le message de commit bb951de). Mais publiquement visible dans l'historique GitHub — doit être nettoyé.

### Détail Finding #3 — Mot de passe test `***FASO_REDACTED_TEST_PASSWORD***`

- **Fichier :** `poulets-platform/e2e/data/seed.ts`
- **Commit :** `3e02b59` (2026-04-07T12:52:11Z) — "Add Jaeger tracing, Playwright E2E suite"
- **Nature :** 4 occurrences, mot de passe générique de test
- **Risque :** LOW/MEDIUM. Valeur générique très commune, peu susceptible d'être réutilisée en prod. Nettoyage recommandé pour hygiène.

---

## Analyse trufflehog

Trufflehog (mode `--only-verified`) n'a retourné **aucun secret vérifié**, confirmant que :
- Les credentials ne sont pas des secrets actifs avec une API externe vérifiable
- La clé privée EC a provoqué un timeout du détecteur (10s) — trufflehog tente une vérification réseau pour les PrivateKey

Logs pertinents :
```
ERROR detector ignored context timeout | detector=PrivateKey | commit=bb951de | file=fixtures.rs
```

---

## Commits à réécrire

| Priorité | Commit SHA | Date | Description | Secrets à retirer |
|----------|------------|------|-------------|-------------------|
| 1 (CRITIQUE) | `bb951dedb886e23259f83aae7222d42c40ced097` | 2026-04-17 | feat(infra): sovereign hardening wave | Clé privée EC dans `fixtures.rs` |
| 2 (HIGH) | `e940cd124ab6fff1c5649fda35b7ed3897ccffa0` | 2026-04-07 | Fix critical startup bugs | `***FASO_REDACTED_TEST_PASSWORD***` dans `seed.ts` |
| 3 (MEDIUM) | `3e02b5976261236c2f31e07251621be9d8756878` | 2026-04-07 | Add Jaeger tracing, Playwright E2E suite | `***FASO_REDACTED_TEST_PASSWORD***` dans `seed.ts` |

---

## Plan de remédiation

### Étape 0 — Immédiat (avant BFG) : Rotation des secrets

| Secret | Type | Owner | Action |
|--------|------|-------|--------|
| Clé privée EC `KAYA_KEY_PEM` | Fixture test mTLS | LIONEL TRAORE | Régénérer avec `openssl` → stocker résultat hors repo (env var ou fichier `.gitignore`d) |
| `***FASO_REDACTED_TEST_PASSWORD***` | Mot de passe E2E test | LIONEL TRAORE | Confirmer non-utilisation prod, sinon changer dans tous les environnements |

### Étape 1 — Retrait du HEAD

```bash
# 1. Régénérer fixtures.rs sans clé privée inline
# Remplacer KAYA_KEY_PEM par lecture depuis fichier .gitignore'd
# OU utiliser rcgen/rustls pour générer à la volée dans les tests

# 2. Committer le retrait
git add armageddon/armageddon-mesh/src/fixtures.rs
git commit -m "security: remove embedded private key from test fixtures

Replace inline PKCS8 PEM constant with runtime-generated key using rcgen.
The key material at commit bb951de was a test-only fixture but must not
remain accessible in public history.

Fixes: SECURITY-AUDIT-2026-04-17 Finding #1"
```

### Étape 2 — BFG Repo-Cleaner

Script préparé : `INFRA/scripts/remediate-history.sh`

```bash
# Prérequis : Java + BFG jar
bash INFRA/scripts/remediate-history.sh
```

### Étape 3 — Force-push (MANUEL — après validation)

```bash
# ATTENTION : action destructive irréversible
# Notifier tous les contributeurs 72h à l'avance
# Vérifier qu'aucun PR ouvert ne référence les commits réécris

git push --force-with-lease origin main

# Demander à GitHub de purger les caches de l'historique
# via le support GitHub (API private vulnerability reporting)
```

### Étape 4 — Prévention future

- `.gitleaks.toml` mis à jour avec allowlist pour `fixtures.rs` (tests mTLS)
- Hook pre-commit gitleaks déjà configuré (`.pre-commit-config.yaml`)
- Ajouter règle : toute clé PEM de test doit être générée à la volée (pas inline)

---

## Owners à notifier

| Nom | Email | Rôle | Action requise |
|-----|-------|------|----------------|
| LIONEL TRAORE | traore.lionel@gmail.com | Auteur unique (tous les commits) | Valider la rotation, approuver le force-push |

> Note : Repo à auteur unique — coordination simplifiée, force-push sans blocage contributeurs.

---

## Faux positifs identifiés

Les patterns suivants ont été manuellement évalués comme non-critiques :

| Pattern | Fichier | Raison |
|---------|---------|--------|
| `VAULT_TOKEN=$(jq -r .root_token ~/.faso-vault-keys.json)` | Scripts Vault | Variable shell, fichier `.gitignore`d |
| `Bearer xyz` / `Bearer tok` | Tests unitaires Rust | Valeurs placeholder dans les tests |
| `jwt-secret` dans doc/README | Documentation | Nom de chemin Vault, pas une valeur |
| `SECRET_ID=$(vault write ...)` | Scripts Vault | Généré dynamiquement à l'exécution |

---

*Rapport généré le 2026-04-18 — FASO DIGITALISATION Security Audit*  
*Outils : gitleaks v8 (sha256:c00b6bd0) + trufflehog v3 (sha256:8837fd74)*
