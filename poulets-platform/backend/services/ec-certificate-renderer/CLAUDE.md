# ec-certificate-renderer — PDF Certificate Generator

## Overview
Sidecar service for generating official civil registry PDF certificates using Playwright (headless Chromium) and Handlebars templates. Generates A4 documents with QR codes, watermarks, and official formatting.

## Technical Details
- Port: 8800 (HTTP)
- DB: none (stateless)
- Package: `bf.gov.faso.renderer`
- Runtime: Java 21 + WebFlux + Virtual Threads + ZGC + Playwright
- Framework: Spring Boot WebFlux (reactive, Netty)

## Key Components
- `PlaywrightMultiBrowserPool` — N Chromium processes x M pages per browser, page recycling
- `TemplateService` — Pre-compiled Handlebars templates (5 types)
- `PdfRenderService` — Cache check → QR code → Handlebars → Playwright → PDF
- `PdfCacheService` — Caffeine cache with SHA-256 keys (1h TTL, 500 entries)
- `HmacAuthFilter` — HMAC-SHA256 validation on X-Internal-Auth header
- `AssetInliner` — Converts fonts/images to data: URIs at startup

## Templates
- `ACTE_NAISSANCE` — Birth certificate
- `ACTE_MARIAGE` — Marriage certificate
- `ACTE_DECES` — Death certificate
- `ACTE_DIVERS` — Miscellaneous acts
- `PERMIS_PORT_ARMES` — Weapons permit

## API Endpoints
- `POST /render/{templateName}` — Single PDF render (requires X-Internal-Auth)
- `POST /render/batch` — Batch render → ZIP (max 50)
- `GET /health` — Health check (no auth required)
- `GET /actuator/health/readiness` — Kubernetes readiness probe

## Dependencies
- **impression-service** (8108) — only caller, sends template data + auth header
- No database, no Kafka, no gRPC

## Security
- HMAC-SHA256 authentication: `X-Internal-Auth: {timestamp}:{hmac_hex}`
- Secret: `INTERNAL_AUTH_SECRET` env var (default: `dev-renderer-secret`)
- Timestamp drift tolerance: 30 seconds
- All external network requests blocked by Playwright route interceptor

## Build & Test
```bash
cd /Users/oz/Documents/PROJECTS/ETAT-CIVIL/backend
./mvnw clean compile -pl services/ec-certificate-renderer -DskipTests
./mvnw spring-boot:run -pl services/ec-certificate-renderer -DskipTests
```

## Health Check
```bash
curl http://localhost:8800/health
```
