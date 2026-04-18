#!/usr/bin/env bash
# =============================================================================
# init-secrets.sh - Generate local dev secrets (never commit the output files)
# =============================================================================
set -euo pipefail

SECRETS_DIR="$(cd "$(dirname "$0")/../secrets" && pwd)"

generate_secret() {
  local file="$SECRETS_DIR/$1"
  local maxlen="${2:-0}"
  if [[ -f "$file" ]]; then
    echo "[skip] $1 already exists"
    return
  fi
  local value
  value=$(openssl rand -base64 32 | tr -d '\n')
  if [[ "$maxlen" -gt 0 ]]; then
    value="${value:0:$maxlen}"
  fi
  printf '%s' "$value" > "$file"
  # 0644 required because kratos (uid 10000) and keto (uid 100) run as non-root
  # inside dev containers and must read the bind-mounted file. Do NOT use 0600.
  chmod 644 "$file"
  echo "[ok]   $1 generated (${#value} bytes)"
}

echo "Generating secrets in: $SECRETS_DIR"
generate_secret postgres_password.txt
generate_secret kratos_cookie_secret.txt
# kratos requires cipher length <= 32 bytes (xchacha20-poly1305)
generate_secret kratos_cipher_secret.txt 32
generate_secret keto_secret.txt

echo ""
echo "Done. These files are git-ignored. Re-run to skip already-generated secrets."
