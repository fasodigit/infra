<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

# Contribuer à FASO DIGITALISATION

Merci pour votre intérêt. Ce projet construit l'infrastructure numérique
souveraine du Burkina Faso — chaque contribution compte.

## Avant de commencer

1. Lire [`README.md`](README.md) et [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md)
2. Consulter la roadmap : https://github.com/fasodigit/infra/projects
3. Ouvrir une **issue de discussion** avant toute PR non triviale

## Workflow

```bash
# 1. Fork + clone
gh repo fork fasodigit/infra --clone
cd infra

# 2. Branche dédiée
git checkout -b feat/<ma-feature>

# 3. Environnement dev (reproductible)
# VS Code : Reopen in Container (.devcontainer/ — Rust 1.83 + Java 21 + Node 22 + Bun)
# Ou local : voir INFRA/.devcontainer/README.md

# 4. Installer les hooks (une seule fois)
pre-commit install

# 5. Développer…
```

## Conventional Commits (obligatoire)

Format : `<type>(<scope>): <description>`

Types autorisés : `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`,
`build`, `ci`, `chore`, `revert`, `sec`.

Scopes usuels : `kaya`, `armageddon`, `auth-ms`, `poulets-api`, `bff`, `frontend`,
`xds`, `observability`, `spire`, `notifier`, `docs`, `ci`, `chaos`.

Un commit avec `BREAKING CHANGE:` déclenche un bump majeur via semantic-release.

**Exemples valides** :

```
feat(kaya): add TOPK.REMOVE command
fix(armageddon): constant-time compare on AUTH password (CVE-2026-XXXXX)
perf(poulets-api): reduce N+1 queries on /api/commandes
```

Bloqué par `commitlint` pre-commit hook.

## Qualité code

| Langage | Commande requise |
|---------|-------------------|
| Rust | `cargo fmt` + `cargo clippy --all-targets -- -D warnings` + `cargo nextest run` + `cargo audit` |
| Java | `mvn verify` (inclut spotbugs + owasp-dependency-check) |
| TypeScript / Angular | `bun run lint` + `bun run test` + `bun run build` |
| Protobuf | `buf lint` + `buf breaking --against=main` |
| Docker | `hadolint Containerfile*` + `trivy fs .` |
| Shell | `shellcheck **/*.sh` |

Tout est automatisé par les hooks `pre-commit`.

## SPDX

Chaque nouveau fichier source doit porter le header :

```rust
// SPDX-License-Identifier: AGPL-3.0-or-later
```

Injection auto via `scripts/spdx-headers.sh` (pré-commit).

## Tests

- **Tests unitaires** : obligatoires pour toute nouvelle fonction publique (≥ 3 cas)
- **Tests d'intégration** : pour toute nouvelle API réseau
- **Tests E2E Playwright** : pour tout nouveau flow utilisateur
- **Benchmarks Criterion** : pour tout changement hot-path (régression > 5 % bloque la PR via bencher.dev)
- **SLO validation** : une nouvelle route = une nouvelle ligne dans `observability/slo/*.slo.yaml`

## Revue

- Toute PR est revue par au moins un mainteneur avant merge
- Les PR touchant à la sécurité requièrent 2 revues (dont une de l'équipe sec)
- Les PR touchant aux schémas Protobuf requièrent validation du Schema Registry Ownership
- Pas de force-push sur `main` / `release/*` (branch protection active)

## Licence des contributions

En ouvrant une PR, vous acceptez de licencier votre contribution sous
**AGPL-3.0-or-later**. Vous certifiez que vous avez le droit de le faire
(propriété ou autorisation de l'employeur).

## Reporting de bugs / vulnérabilités

- **Bug fonctionnel** → issue publique avec template `.github/ISSUE_TEMPLATE/bug.md`
- **Vulnérabilité** → **jamais en public**, voir [`SECURITY.md`](SECURITY.md)

## Demande de nouvelle fonctionnalité

- Ouvrir une discussion GitHub d'abord (« RFC »)
- Une fois consensus, créer une issue + PR référencée

## Reconnaissance

Les contributeurs sont listés dans `ACKNOWLEDGMENTS.md` (auto via CI).

---

*Merci pour votre temps. Le Burkina Faso vous en sait gré.*
