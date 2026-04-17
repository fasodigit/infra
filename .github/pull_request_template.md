<!-- SPDX-License-Identifier: AGPL-3.0-only -->
<!-- FASO DIGITALISATION — Pull Request Template -->

## Description

<!--
Describe the changes and their motivation.
Link related issues: "Closes #123" or "Fixes #456"
-->

## Conventional Commits Reminder

Every commit message **must** follow the [Conventional Commits](https://www.conventionalcommits.org/) specification to drive semantic versioning automatically.

| Prefix | Release bump | Example |
|--------|-------------|---------|
| `feat:` | **minor** (0.x.0) | `feat(kaya): add WAL compaction strategy` |
| `fix:` | **patch** (0.0.x) | `fix(auth-ms): handle expired JWT tokens` |
| `perf:` | **patch** | `perf(armageddon): optimise WASM JIT cache` |
| `BREAKING CHANGE:` footer | **major** (x.0.0) | body footer: `BREAKING CHANGE: renamed config key` |
| `chore:`, `docs:`, `ci:`, `test:`, `refactor:`, `style:` | **no release** | |

> To skip a release entirely: `chore(release): skip`
> To include breaking change: add `BREAKING CHANGE: <description>` in commit footer

## Checklist

- [ ] Commits follow Conventional Commits (`feat:`, `fix:`, `chore:`, etc.)
- [ ] Breaking changes documented in commit footer (`BREAKING CHANGE: ...`) and in PR description
- [ ] SPDX license header present in new files (`// SPDX-License-Identifier: AGPL-3.0-only`)
- [ ] Tests added/updated for the changes
- [ ] CI passes (Rust / Java / Node pipelines)
- [ ] CHANGELOG.md will be auto-updated by semantic-release — do not edit manually
- [ ] No tags have been manually deleted or altered
