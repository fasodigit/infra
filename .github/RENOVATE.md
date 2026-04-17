# Renovate Configuration

Renovate is the primary dependency manager for this workspace. Dependabot is kept as a fallback for GitHub Actions and Docker security alerts only.

## Schedule

Updates are proposed **before 06:00 on weekdays**.

## Auto-merge Policy

| Update type | Condition | Auto-merge |
|---|---|---|
| `patch` | CI green | Yes |
| `minor` | CI green | Yes |
| `major` (dev deps only) | CI green | Yes |
| `major` (runtime deps) | — | No (manual review) |
| Any with RustSec advisory | — | **Blocked** — labeled `security` |

## Grouped Packages

- **tokio** — all `tokio*` crates
- **serde** — all `serde*` crates
- **tracing** — all `tracing*` crates
- **opentelemetry** — all `opentelemetry*` crates
- **rustls** — all `rustls*` crates
- **scripting-runtimes** — `rhai`, `wasmtime`, `mlua`

## Security Workflow

`cargo-audit` runs on every push, PR, and daily at 02:00 UTC. A PR is blocked if a `RUSTSEC-*` advisory is found unless the maintainer adds the label **`security-accepted`** and documents the accepted risk in `.cargo/audit.toml`.

`cargo-deny` additionally enforces license compliance (GPL-2.0 banned, AGPL-3.0/MIT/Apache-2.0/BSD-* allowed) and source trust.
