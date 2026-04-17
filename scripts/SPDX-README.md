# SPDX License Header Tooling — FASO DIGITALISATION

## Overview

All FASO source files must carry an **AGPL-3.0-or-later** SPDX identifier header.
Two scripts enforce this: one injects headers, one validates them (used in CI).

---

## Scripts

### `spdx-headers.sh` — Injection (developer tool)

Injects SPDX headers into files that are missing them. **Idempotent** — safe to re-run.

```bash
# Inject headers (modifies files)
bash INFRA/scripts/spdx-headers.sh

# Preview what would be changed (no modifications)
bash INFRA/scripts/spdx-headers.sh --dry-run
```

#### Header format by language

| Language | Pattern | Header |
|---|---|---|
| Rust (`*.rs`) | `// SPDX...` (2 lines) | `// SPDX-License-Identifier: AGPL-3.0-or-later` + copyright |
| Java (`*.java`) | `/* ... */` block | `/* SPDX-License-Identifier: AGPL-3.0-or-later ... */` |
| TypeScript/TSX | `// SPDX...` (2 lines) | Same as Rust |
| Protobuf (`*.proto`) | `// SPDX...` (2 lines) | Same as Rust |

#### Covered paths

- `INFRA/kaya/src/**/*.rs`
- `INFRA/armageddon/**/*.rs` (all crates)
- `INFRA/auth-ms/src/**/*.java`
- `INFRA/poulets-platform/backend/src/**/*.java`
- `INFRA/poulets-platform/frontend/src/**/*.ts` / `*.tsx`
- `INFRA/poulets-platform/backend/src/main/proto/**/*.proto`
- `INFRA/kaya/**/*.proto`

#### Always skipped

`node_modules/`, `target/`, `dist/`, `build/`, `generated/`, `gen/`, `vendor/`, `third_party/`, `*.d.ts`

---

### `spdx-check.sh` — Validation (CI tool)

Exits **0** if all covered files have headers, **1** otherwise (with a list of offenders).

```bash
bash INFRA/scripts/spdx-check.sh
```

---

## CI Integration

**GitHub Actions workflow**: `.github/workflows/spdx-check.yml`

Triggers on every PR targeting `main`, `develop`, or `release/**` when relevant source files change. The job blocks merge on failure.

---

## Pre-commit Hook (standalone)

`INFRA/.pre-commit-spdx.yaml` is a **separate** pre-commit configuration that does **not** modify the global `.pre-commit-config.yaml` (to avoid conflicts with Agent #2 Pre-commit work).

```bash
# Run manually against all files
pre-commit run --config INFRA/.pre-commit-spdx.yaml --all-files

# Install as a git hook (standalone)
pre-commit install --config INFRA/.pre-commit-spdx.yaml
```

To merge into the global config later, copy the `repos` entry from `.pre-commit-spdx.yaml` into `.pre-commit-config.yaml`.

---

## Typical developer workflow

```bash
# 1. Code your feature
# 2. Inject headers if needed
bash INFRA/scripts/spdx-headers.sh
# 3. Verify (same check as CI)
bash INFRA/scripts/spdx-check.sh
# 4. Commit
```
