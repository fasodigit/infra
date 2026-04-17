<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

# Politique de sécurité — FASO DIGITALISATION

## Engagement

La sécurité du code et des données des citoyens du Burkina Faso est
notre priorité absolue. Toute vulnérabilité est traitée avec le plus haut
niveau de diligence, dans la transparence et avec coordination avec les
personnes qui la signalent.

## Signaler une vulnérabilité

**Ne jamais ouvrir d'issue publique pour une vulnérabilité.**

### Canaux de contact préférés

| Canal | Adresse | Usage |
|-------|---------|-------|
| GitHub Private Vulnerability Reporting | https://github.com/fasodigit/infra/security/advisories/new | ✅ préféré |
| Email chiffré PGP | `security@faso.gov.bf` (clé PGP : [TODO à publier]) | alternatives |
| Encrypted Signal / WhatsApp | contact via l'email ci-dessus pour obtenir le numéro | urgences critiques |

Merci d'inclure :

- Description de la vulnérabilité et composant affecté (KAYA / ARMAGEDDON / auth-ms / etc.)
- Étapes reproductibles (PoC minimal)
- Impact estimé (CVSS v3.1 si possible)
- Version(s) concernée(s) (commit SHA)
- Vos coordonnées (pour attribution — optionnel)

## Engagement de réponse

| Phase | Délai cible |
|-------|-------------|
| Accusé de réception | ≤ 48 h ouvrées |
| Triage initial (criticité + priorité) | ≤ 5 j ouvrés |
| Correctif proposé ou plan de mitigation | ≤ 14 j (Critical), ≤ 30 j (High), ≤ 90 j (Medium) |
| Divulgation coordonnée | après déploiement correctif + période de grâce 7 j |

## Périmètre

### Dans le périmètre

- Tous les crates Rust (KAYA, ARMAGEDDON, xds-controller)
- Tous les microservices Java (auth-ms, poulets-api, notifier-ms)
- BFF Next.js + frontend Angular
- Configurations Docker/Podman, Kubernetes, ORY Kratos/Keto
- Workflows GitHub Actions et scripts CI/CD
- Dépendances tierces avec vulnérabilités non-déclarées

### Hors périmètre

- Vulnérabilités dans des dépendances amont déjà rapportées (liens CVE requis)
- Social engineering, phishing d'agents
- DDoS sur infrastructures tierces non contrôlées
- Attaques physiques sur data-centers

## Reconnaissance

Les personnes signalant de manière responsable peuvent être listées dans
`ACKNOWLEDGMENTS.md` (sauf demande d'anonymat). Un programme de bug-bounty
sera annoncé courant 2026.

## Vulnérabilités connues

Les vulnérabilités divulguées publiquement sont publiées dans :

- GitHub Security Advisories : https://github.com/fasodigit/infra/security/advisories
- OSS-Fuzz (KAYA) : à venir
- Rustsec advisories : dépendances auditées via `cargo audit` (CI quotidien)

## Architecture de défense en profondeur

- **mTLS SPIRE** partout — rotation 24 h, alertes < 72 h
- **ARMAGEDDON** — WAF Pentagon (IPS/WAF/ML/Rego/AI) sur chaque requête entrante
- **KAYA** — AUTH constant-time, Rhai sandbox limité, frame size caps RESP3
- **Docker secrets** — jamais en clair dans compose.yml
- **SPDX AGPL** — chaque fichier source signe sa licence
- **CI** — cargo-audit + cargo-deny + Trivy + Grype + SBOM CycloneDX sur chaque PR
- **Chaos Mesh** — kill KAYA replica nightly, partition réseau, clock skew

## Clé publique PGP

À publier. En attendant, utilisez GitHub Private Vulnerability Reporting.

---

*Merci de protéger les citoyens qui nous font confiance.*
