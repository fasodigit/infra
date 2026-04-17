#!/usr/bin/env bash
# =============================================================================
# init-secrets.sh - Generate local dev secrets (never commit the output files)
# =============================================================================
set -euo pipefail

SECRETS_DIR="$(cd "$(dirname "$0")/../secrets" && pwd)"

generate_secret() {
  local file="$SECRETS_DIR/$1"
  if [[ -f "$file" ]]; then
    echo "[skip] $1 already exists"
    return
  fi
  openssl rand -base64 32 | tr -d '\n' > "$file"
  chmod 600 "$file"
  echo "[ok]   $1 generated"
}

echo "Generating secrets in: $SECRETS_DIR"
generate_secret postgres_password.txt
generate_secret kratos_cookie_secret.txt
generate_secret kratos_cipher_secret.txt
generate_secret keto_secret.txt

echo ""
echo "Done. These files are git-ignored. Re-run to skip already-generated secrets."
