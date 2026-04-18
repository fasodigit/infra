# impression-service — Document Printing

## Overview
Manages the printing pipeline for validated civil registry documents. Generates print-ready documents with QR codes, watermarks, and anti-forgery security features. Tracks printer assignments and print job status.

## Technical Details
- Port: 8130 (HTTP), 9130 (gRPC)
- DB: none (stateless, relies on upstream data)
- Package: `bf.gov.etatcivil.impression`
- Runtime: Java 21 + Virtual Threads + ZGC
- GraphQL: DGS Framework (Apollo Federation subgraph)

## Key Entities
- `JobImpression` — Print job with status tracking (EN_ATTENTE, EN_COURS, TERMINE, ECHOUE)
- `QRCodeActe` — QR code metadata embedded in printed documents for verification
- `Filigrane` — Watermark configuration per document type
- `SecuriteAntiContrefacon` — Anti-forgery features applied to the document

## Dependencies
- **validation-acte-service** (8120) — source validated documents
- **document-security-ms** (8160) — QR code generation, watermark application, anti-forgery features
- **actor-ms** (8200) — operator identity and role verification (IMPRESSION role)
- **document-storage-service** (8150) — retrieve validated document files

## Redpanda Topics
- Produces: `etatcivil.acte.imprime`, `etatcivil.impression.echouee`
- Consumes: `etatcivil.acte.valide` (validated documents ready for printing)

## DragonflyDB Keys (DB0, prefix: ec:)
- `ec:impression:queue:{tenantId}` — print queue per tenant
- `ec:impression:job:{jobId}` — print job status cache, TTL 1h
- `ec:impression:qr:{acteId}` — QR code verification data, TTL 24h

## Business Rules
- Only validated (WORM-sealed) documents can be printed
- Each printed document must include a unique QR code for authenticity verification
- Watermarks are applied based on document type and issuing authority
- Only actors with IMPRESSION role can trigger and manage print jobs
- Failed print jobs are automatically requeued up to 3 times before manual intervention
- Print audit trail must record operator, timestamp, printer ID, and number of copies

## Build & Test
```bash
cd /Users/oz/Documents/PROJECTS/ETAT-CIVIL/backend
./mvnw clean compile -pl services/impression-service -DskipTests
./mvnw test -pl services/impression-service
```

---

## Workflow Orchestration

### 1. Plan Node Default
- Enter plan mode for ANY non-trivial task (3+ steps or architectural decisions)
- If something goes sideways, STOP and re-plan immediately — don't keep pushing
- Use plan mode for verification steps, not just building
- Write detailed specs upfront to reduce ambiguity

### 2. Subagent Strategy
- Use subagents liberally to keep main context window clean
- Offload research, exploration, and parallel analysis to subagents
- For complex problems, throw more compute at it via subagents
- One task per subagent for focused execution

### 3. Self-Improvement Loop
- After ANY correction from the user: update `tasks/lessons.md` with the pattern
- Write rules for yourself that prevent the same mistake
- Ruthlessly iterate on these lessons until mistake rate drops
- Review lessons at session start for relevant project

### 4. Verification Before Done
- Never mark a task complete without proving it works
- Diff behavior between main and your changes when relevant
- Ask yourself: "Would a staff engineer approve this?"
- Run tests, check logs, demonstrate correctness

### 5. Demand Elegance (Balanced)
- For non-trivial changes: pause and ask "is there a more elegant way?"
- If a fix feels hacky: rethink and implement the elegant solution
- Skip this for simple, obvious fixes — don't over-engineer
- Challenge your own work before presenting it

### 6. Autonomous Bug Fixing
- When given a bug report: just fix it. Don't ask for hand-holding
- Point at logs, errors, failing tests — then resolve them
- Zero context switching required from the user
- Go fix failing CI tests without being told how

## Task Management

1. **Plan First**: Write plan to `tasks/todo.md` with checkable items
2. **Verify Plan**: Check in before starting implementation
3. **Track Progress**: Mark items complete as you go
4. **Explain Changes**: High-level summary at each step
5. **Document Results**: Add review section to `tasks/todo.md`
6. **Capture Lessons**: Update `tasks/lessons.md` after corrections

## Core Principles

- **Simplicity First**: Make every change as simple as possible. Impact minimal code.
- **No Laziness**: Find root causes. No temporary fixes. Senior developer standards.
- **Minimal Impact**: Changes should only touch what's necessary. Avoid introducing bugs.
