<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## Contexte

<!-- Quel problème / besoin motive cette PR ? Lien vers issue / RFC si pertinent. -->

## Changements

<!-- Liste atomique de ce qui change, par scope. Aligne sur les conventional commits. -->

- 

## Type de PR

- [ ] `feat`: nouvelle fonctionnalité
- [ ] `fix`: correction de bug
- [ ] `chore`: refactor / tooling / pas de feature ni bug
- [ ] `docs`: documentation pure
- [ ] `test`: ajout/modif tests
- [ ] `perf`: amélioration de performance
- [ ] `security`: correctif sécurité (utiliser Private Vulnerability Reporting si non-publié)

## Test plan

<!-- Liste des vérifications que tu as faites localement / en CI. -->

- [ ] `cargo nextest run` (workspace touché)
- [ ] `cargo clippy -D warnings`
- [ ] `mvn verify` (microservice touché)
- [ ] `bun run lint && bun run test`
- [ ] Stack GREEN via `/cycle-fix` (cf. CLAUDE.md §10)
- [ ] Spec Playwright correspondante écrite (cf. CLAUDE.md §11)
- [ ] Pre-commit hooks passés sans `--no-verify`

## Risque & rollback

<!-- Quelle est la blast-radius si ça casse ? Comment rollback ? -->

## Checklist mainteneur

- [ ] Branch protection à jour (Required checks / all-green vert)
- [ ] CHANGELOG.md mis à jour si breaking change
- [ ] CODEOWNERS notifié (auto)
- [ ] Trailer `Co-Authored-By` présent si commit généré par agent IA
