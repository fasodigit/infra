#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 FASO DIGITALISATION
#
# remediate-history.sh — Nettoyage historique git avec BFG Repo-Cleaner
#
# Usage:
#   bash scripts/remediate-history.sh [--dry-run]
#
# Prérequis:
#   - Java 11+ installé (java -version)
#   - BFG jar téléchargé : https://rtyley.github.io/bfg-repo-cleaner/
#     → placer dans scripts/bfg.jar OU définir BFG_JAR=/chemin/vers/bfg.jar
#   - Être sur la branche main, working tree PROPRE (git status = clean)
#   - Avoir retiré les secrets du HEAD AVANT d'exécuter ce script
#
# AVERTISSEMENT : Ce script réécrit l'historique git.
#   - Ne PAS exécuter le force-push automatiquement (section commentée)
#   - Notifier tous les contributeurs 72h avant le force-push
#   - Créer un backup complet du repo avant exécution
#
# Référence : SECURITY-AUDIT-2026-04-17.md
# Commits ciblés :
#   - bb951dedb886e23259f83aae7222d42c40ced097 (clé privée EC dans fixtures.rs)
#   - e940cd124ab6fff1c5649fda35b7ed3897ccffa0 (***FASO_REDACTED_TEST_PASSWORD*** dans seed.ts)
#   - 3e02b5976261236c2f31e07251621be9d8756878 (***FASO_REDACTED_TEST_PASSWORD*** dans seed.ts)

set -euo pipefail

# ─────────────────────────────────────────────────────────────────
# Configuration
# ─────────────────────────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BFG_JAR="${BFG_JAR:-$SCRIPT_DIR/bfg.jar}"
PATTERNS_FILE="$SCRIPT_DIR/bfg-patterns.txt"
DRY_RUN=false

# Couleurs
RED='\033[0;31m'
YELLOW='\033[1;33m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

# ─────────────────────────────────────────────────────────────────
# Parsing arguments
# ─────────────────────────────────────────────────────────────────
for arg in "$@"; do
  case $arg in
    --dry-run) DRY_RUN=true ;;
    --help|-h)
      grep '^#' "$0" | sed 's/^# \?//' | head -30
      exit 0
      ;;
  esac
done

# ─────────────────────────────────────────────────────────────────
# Fonctions utilitaires
# ─────────────────────────────────────────────────────────────────
info()    { echo -e "${BLUE}[INFO]${NC}  $*"; }
warn()    { echo -e "${YELLOW}[WARN]${NC}  $*"; }
error()   { echo -e "${RED}[ERROR]${NC} $*" >&2; }
success() { echo -e "${GREEN}[OK]${NC}    $*"; }

confirm() {
  local prompt="$1"
  echo -e "${YELLOW}${prompt} [y/N]${NC} " >&2
  read -r answer
  [[ "${answer,,}" == "y" ]]
}

# ─────────────────────────────────────────────────────────────────
# Vérifications préalables
# ─────────────────────────────────────────────────────────────────
info "=== FASO DIGITALISATION — BFG History Remediation ==="
info "Repo : $REPO_ROOT"
info "Dry-run : $DRY_RUN"
echo ""

# Vérifier Java
if ! command -v java &>/dev/null; then
  error "Java non trouvé. Installer Java 11+ : sudo apt install openjdk-21-jre"
  exit 1
fi
JAVA_VERSION=$(java -version 2>&1 | head -1)
info "Java : $JAVA_VERSION"

# Vérifier BFG
if [[ ! -f "$BFG_JAR" ]]; then
  warn "BFG jar non trouvé à : $BFG_JAR"
  warn "Téléchargement automatique..."
  if command -v wget &>/dev/null; then
    wget -q -O "$BFG_JAR" \
      "https://repo1.maven.org/maven2/com/madgag/bfg/1.14.0/bfg-1.14.0.jar"
  elif command -v curl &>/dev/null; then
    curl -sSL -o "$BFG_JAR" \
      "https://repo1.maven.org/maven2/com/madgag/bfg/1.14.0/bfg-1.14.0.jar"
  else
    error "Ni wget ni curl disponible. Télécharger manuellement :"
    error "  https://rtyley.github.io/bfg-repo-cleaner/"
    error "  → placer dans $BFG_JAR"
    exit 1
  fi
  success "BFG téléchargé : $BFG_JAR"
fi

# Vérifier working tree propre
cd "$REPO_ROOT"
if [[ -n "$(git status --porcelain 2>/dev/null)" ]]; then
  error "Working tree non propre. Committer ou stasher les changements avant de continuer."
  git status --short
  exit 1
fi
success "Working tree propre"

# Vérifier que les secrets ne sont PLUS dans HEAD
info "Vérification que les secrets sont absents du HEAD..."
if git show HEAD:armageddon/armageddon-mesh/src/fixtures.rs 2>/dev/null | grep -q "BEGIN PRIVATE KEY"; then
  error "La clé privée est encore dans HEAD (fixtures.rs)."
  error "Retirer la clé du HEAD et committer AVANT d'exécuter BFG."
  error "BFG préserve toujours le contenu du HEAD — il ne peut pas nettoyer ce qui est encore présent."
  exit 1
fi
if git show HEAD:poulets-platform/e2e/data/seed.ts 2>/dev/null | grep -q "FasoP0ulet\|Test1234"; then
  error "Un mot de passe de test est encore dans HEAD (seed.ts)."
  error "Retirer du HEAD et committer AVANT d'exécuter BFG."
  exit 1
fi
success "Secrets absents du HEAD — BFG peut procéder"

# ─────────────────────────────────────────────────────────────────
# Création du fichier patterns.txt
# ─────────────────────────────────────────────────────────────────
info "Génération de $PATTERNS_FILE..."
cat > "$PATTERNS_FILE" <<'PATTERNS'
# BFG replacement patterns
# Format: REGEX==>REPLACEMENT  (ou juste REGEX pour suppression totale)
# Ref: https://rtyley.github.io/bfg-repo-cleaner/#replace-expressions

# Finding #1 — Clé privée EC PKCS#8 (fixtures.rs)
# Note: BFG remplace la valeur littérale par ***REMOVED***
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgBmB6gFXFqw8B7\+mB==>***REMOVED***
LisTrGjk6vqrZuMFn0P3bF4IJ4KhRANCAASYVpUg\+RCXRXxNsFk3EGfyFQFUko9U==>***REMOVED***
CYaNdLeLrAnZ0ueNdnwoPlpYREsAc0AioDcwyQJ2bSvuRVQg\+Yjskakl==>***REMOVED***

# Finding #2 — Mot de passe E2E primaire
FasoP0ulet\$2026Xk9m==>***REMOVED***

# Finding #3 — Mot de passe E2E générique
Test1234!\@#\$==>***REMOVED***
PATTERNS
success "Patterns créés : $PATTERNS_FILE"

# ─────────────────────────────────────────────────────────────────
# Backup
# ─────────────────────────────────────────────────────────────────
BACKUP_DIR="/tmp/faso-infra-backup-$(date +%Y%m%d-%H%M%S)"
info "Création d'un backup complet dans $BACKUP_DIR..."
if [[ "$DRY_RUN" == "false" ]]; then
  git clone --mirror "$REPO_ROOT" "$BACKUP_DIR" 2>/dev/null || {
    cp -r "$REPO_ROOT/.git" "$BACKUP_DIR"
  }
  success "Backup créé : $BACKUP_DIR"
else
  info "[DRY-RUN] Backup ignoré"
fi

# ─────────────────────────────────────────────────────────────────
# Affichage du plan
# ─────────────────────────────────────────────────────────────────
echo ""
warn "=== PLAN D'EXÉCUTION BFG ==="
warn "Commits ciblés :"
warn "  bb951de — clé privée EC (fixtures.rs)"
warn "  e940cd1 — FasoP0ulet\$2026Xk9m (seed.ts)"
warn "  3e02b59 — Test1234!@#\$ (seed.ts)"
warn ""
warn "Patterns à effacer : $PATTERNS_FILE"
warn "BFG jar : $BFG_JAR"
echo ""

if [[ "$DRY_RUN" == "false" ]]; then
  if ! confirm "Confirmer l'exécution BFG (réécriture d'historique) ?"; then
    info "Annulé par l'utilisateur."
    exit 0
  fi
fi

# ─────────────────────────────────────────────────────────────────
# Exécution BFG
# ─────────────────────────────────────────────────────────────────
if [[ "$DRY_RUN" == "true" ]]; then
  info "[DRY-RUN] Commande BFG qui serait exécutée :"
  echo "  java -jar $BFG_JAR --replace-text $PATTERNS_FILE $REPO_ROOT/.git"
  echo ""
  info "[DRY-RUN] Commandes de nettoyage qui suivraient :"
  echo "  git reflog expire --expire=now --all"
  echo "  git gc --prune=now --aggressive"
else
  info "Exécution BFG..."
  java -jar "$BFG_JAR" \
    --replace-text "$PATTERNS_FILE" \
    "$REPO_ROOT/.git" \
    2>&1 | tee /tmp/bfg-run-$(date +%Y%m%d-%H%M%S).log

  BFG_EXIT=${PIPESTATUS[0]}
  if [[ $BFG_EXIT -ne 0 ]]; then
    error "BFG a échoué (exit $BFG_EXIT). Consulter le log ci-dessus."
    exit $BFG_EXIT
  fi
  success "BFG terminé"

  # ─────────────────────────────────────────────────────────────────
  # Nettoyage des refs orphelines
  # ─────────────────────────────────────────────────────────────────
  info "Nettoyage reflog et GC..."
  cd "$REPO_ROOT"
  git reflog expire --expire=now --all
  git gc --prune=now --aggressive
  success "Nettoyage terminé"

  # ─────────────────────────────────────────────────────────────────
  # Vérification post-BFG
  # ─────────────────────────────────────────────────────────────────
  info "Vérification post-BFG — recherche des patterns dans l'historique..."
  FOUND=0

  if git log --all -p 2>/dev/null | grep -q "MIGHAgEAMBMGByqGSM49"; then
    error "ÉCHEC : clé privée encore présente dans l'historique !"
    FOUND=$((FOUND + 1))
  else
    success "Clé privée : absente de l'historique"
  fi

  if git log --all -p 2>/dev/null | grep -q "FasoP0ulet"; then
    error "ÉCHEC : FasoP0ulet\$2026Xk9m encore présent dans l'historique !"
    FOUND=$((FOUND + 1))
  else
    success "FasoP0ulet\$2026Xk9m : absent de l'historique"
  fi

  if git log --all -p 2>/dev/null | grep -q "Test1234!\@"; then
    warn "Test1234!@#\$ encore présent dans l'historique (peut être un faux positif)"
  else
    success "Test1234!@#\$ : absent de l'historique"
  fi

  if [[ $FOUND -gt 0 ]]; then
    error "$FOUND pattern(s) encore présent(s). BFG n'a pas tout nettoyé."
    error "Vérifier les patterns dans $PATTERNS_FILE et re-exécuter."
    exit 1
  fi
fi

# ─────────────────────────────────────────────────────────────────
# Instructions force-push (MANUEL)
# ─────────────────────────────────────────────────────────────────
echo ""
echo -e "${RED}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${RED}║  FORCE-PUSH NON EXÉCUTÉ AUTOMATIQUEMENT — ACTION MANUELLE   ║${NC}"
echo -e "${RED}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""
warn "Avant le force-push :"
warn "  1. Notifier les contributeurs 72h à l'avance (email + GitHub issue)"
warn "  2. S'assurer qu'aucun PR ouvert ne référence les commits réécris"
warn "  3. Vérifier que le backup est intact : $BACKUP_DIR"
warn "  4. Faire valider par un second pair"
echo ""
info "Commande force-push (à exécuter MANUELLEMENT après validation) :"
echo ""
echo -e "  ${RED}git push --force-with-lease origin main${NC}"
echo ""
info "Après le force-push :"
echo "  - Contacter GitHub Support pour purger les caches CDN"
echo "  - Révoquer et régénérer tout token/credential potentiellement exposé"
echo "  - Mettre à jour SECURITY.md avec la date de remédiation"
echo "  - Ajouter les fixtures à l'allowlist .gitleaks.toml"
echo ""
success "=== Script terminé. Vérifier les résultats ci-dessus avant tout push. ==="
