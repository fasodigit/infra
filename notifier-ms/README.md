# notifier-ms

**FASO DIGITALISATION — Notification Microservice**

Consumes GitHub webhook events from the `github.events.v1` topic (produced by ARMAGEDDON),
resolves contextual Handlebars templates per repository/event type, and dispatches
transactional emails via SMTP (MailHog in dev, Mailersend in prod).

## Architecture

```
ARMAGEDDON webhook handler
        │ (Redpanda topic: github.events.v1)
        ▼
GithubEventConsumer
  ├─ Deduplicate via KAYA (SET NX, 7-day TTL)
  ├─ ContextRulesEngine.evaluate() → matched rules
  └─ NotificationService.dispatchAll() [async]
        ├─ TemplateRenderService (Handlebars)
        ├─ JavaMailSender → SMTP
        ├─ Resilience4j retry (3× exponential)
        └─ On failure → DLQ topic + DB status=DLQ
```

## Maven Modules

| Module | Role |
|---|---|
| `notifier-core` | Consumer, service logic, domain model, Flyway, fat JAR |
| `notifier-api` | REST controllers, Spring Security JWT |
| `notifier-templates` | Handlebars `.hbs` template resources |

## Ports

| Port | Protocol | Description |
|---|---|---|
| 8803 | HTTP | Application API |
| 9090 | HTTP | Actuator (health, prometheus) |
| 9803 | gRPC | Reserved (future) |

## Quick Start (dev)

```bash
# Start dependencies
podman-compose -f INFRA/docker/compose/docker-compose.yml up -d \
  postgres kaya redpanda mailhog

# Run notifier-ms
cd INFRA/notifier-ms
mvn spring-boot:run -pl notifier-core \
  -Dspring-boot.run.profiles=dev \
  -Dspring-boot.run.jvmArguments="--enable-preview"
```

## Templates

All templates live in `notifier-templates/src/main/resources/templates/`:

| Template | Trigger | Recipients |
|---|---|---|
| `infra-commit.hbs` | push → fasodigit/infra | devops@faso.gov.bf |
| `vouchers-commit.hbs` | push → fasodigit/vouchers | agriculture@faso.gov.bf |
| `etatcivil-commit.hbs` | push → fasodigit/etatcivil | etatcivil@faso.gov.bf |
| `poulets-commit.hbs` | push → fasodigit/poulets | poulets@faso.gov.bf |
| `sogesy-commit.hbs` | push → fasodigit/sogesy | sogesy@faso.gov.bf |
| `hospital-commit.hbs` | push → fasodigit/hospital | hospital@faso.gov.bf |
| `escool-commit.hbs` | push → fasodigit/escool | escool@faso.gov.bf |
| `eticket-commit.hbs` | push → fasodigit/eticket | eticket@faso.gov.bf |
| `altmission-commit.hbs` | push → fasodigit/altmission | altmission@faso.gov.bf |
| `fasokalan-commit.hbs` | push → fasodigit/fasokalan | fasokalan@faso.gov.bf |
| `pull-request-opened.hbs` | PR opened → fasodigit/* | devops@faso.gov.bf |
| `pull-request-merged.hbs` | PR merged → fasodigit/* | devops@faso.gov.bf |

## REST API

Base path: `http://notifier-ms:8803/api`
Auth: Bearer JWT (validated against auth-ms JWKS)

| Endpoint | Method | Scope | Description |
|---|---|---|---|
| `/api/templates` | GET | `notifier:read` | List all templates |
| `/api/templates/{name}` | GET/PUT/DELETE | `notifier:read`/`admin` | Template CRUD |
| `/api/deliveries` | GET | `notifier:read` | List deliveries (paginated) |
| `/api/deliveries/{id}/retry` | POST | `notifier:admin` | Retry failed delivery |
| `/api/rules` | GET/PUT | `notifier:read`/`admin` | Context rules management |
| `/api/rules/evaluate` | POST | `notifier:read` | Test rule evaluation |

## Metrics (Prometheus)

| Metric | Description |
|---|---|
| `notifier_mail_sent_total` | Successfully dispatched emails |
| `notifier_mail_failed_total` | Permanently failed deliveries |
| `notifier_template_render_duration_ms` | Handlebars render latency |
| `notifier_dedupe_hit_total` | Duplicates suppressed by KAYA |
| `notifier_dlq_total` | Events forwarded to DLQ |

## SPDX License

`AGPL-3.0-only` — all Java sources carry the SPDX header.
