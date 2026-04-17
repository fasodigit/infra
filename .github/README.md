# FASO DIGITALISATION — Developer Portal

## Documentation Links

| Resource | URL |
|---|---|
| Rustdoc (KAYA) | <https://fasodigit.github.io/infra/kaya/kaya_server/index.html> |
| Rustdoc (ARMAGEDDON) | <https://fasodigit.github.io/infra/armageddon/armageddon/index.html> |
| OpenAPI — auth-ms | <https://fasodigit.github.io/infra/openapi/auth-ms/index.html> |
| OpenAPI — poulets-api | <https://fasodigit.github.io/infra/openapi/poulets-api/index.html> |
| Buf Schema Registry | <https://buf.build/faso-digitalisation> |
| Grafana Dashboards | <https://grafana.fasodigit.io> |

## CI / CD Workflows

| Workflow | Trigger | Purpose |
|---|---|---|
| `rustdoc.yml` | push main — `INFRA/kaya/**` or `INFRA/armageddon/**` | Build & publish Rustdoc to GitHub Pages |
| `openapi-redoc.yml` | push main — `INFRA/auth-ms/**` or `INFRA/poulets-platform/**` | Extract OpenAPI 3, build Redoc HTML, publish to GitHub Pages |
| `markdown-lint.yml` | push / PR on any `*.md` | Lint Markdown with markdownlint-cli2 |
| `cargo-audit.yml` | push / PR / daily | RustSec vulnerability scan |
| `cargo-deny.yml` | push / PR | License & dependency policy check |
| `container-scan.yml` | push / PR | Container image vulnerability scan |

> GitHub Pages base URL: <https://fasodigit.github.io/infra/>
