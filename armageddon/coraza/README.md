# Coraza WAF — FASO inline gateway protection

## Architecture

ARMAGEDDON Pingora charge `coraza-waf.wasm` via le `wasm_adapter` engine.
Le module exécute les règles OWASP CRS v4 + custom FASO en mode inline,
sans round-trip réseau.

## Build

Le `.wasm` est buildé via TinyGo :

```bash
# Pré-requis
go install github.com/tinygo-org/tinygo@v0.34.0

# Build (depuis ce répertoire)
bash build.sh

# Verify
ls -la coraza-waf.wasm
# → coraza-waf.wasm (~10MB)
```

Le `coraza-waf.wasm` est ensuite mounté dans le container ARMAGEDDON ou
distribué dans le binaire pré-build.

## Configuration

| Fichier | Rôle |
|---------|------|
| `coraza.conf` | Config principale + 30 règles CRS critiques + 5 règles FASO custom |
| `crs/` | OWASP CRS v4.10.0 (téléchargé par build.sh) |
| `scanners.txt` | Liste de scanners web bloqués au niveau User-Agent |

## Paranoia levels

Démarrage à PL=1 (faible faux positif). Pour monter à PL=2 après évaluation :

```bash
# Editer coraza.conf, ligne setvar:tx.paranoia_level=1 → 2
# Reload via ARMAGEDDON admin API:
curl -X POST http://127.0.0.1:9902/admin/waf/reload
```

## Tests

Suite `tests-e2e/tests/17-owasp-top10/` couvre :

- A03 SQLi (`?id=' OR 1=1 --`)
- A03 XSS (`<script>alert(1)</script>`)
- A03 Command injection (`?cmd=;cat /etc/passwd`)
- A03 LDAP injection
- A10 SSRF vers private CIDR
- A10 SSRF vers metadata service AWS

Critère d'acceptance : tous les payloads malveillants → 403, latence WAF p99 < 5ms.

## Métriques exposées

- `armageddon_waf_blocks_total{rule_id, severity}` — compteur de blocks
- `armageddon_waf_eval_duration_seconds{rule_id}` — histogramme latence
- `armageddon_waf_anomaly_score` — gauge dernière score par requête
