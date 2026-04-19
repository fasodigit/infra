# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 FASO DIGITALISATION
#
# Synthetic monitoring runner image.
# Base: official Playwright image (Chromium + system deps pre-installed).
# Builds are published as ghcr.io/faso-digitalisation/synthetic-monitoring:<tag>.

FROM mcr.microsoft.com/playwright:v1.48.0-jammy AS base

LABEL org.opencontainers.image.title="faso-synthetic-monitoring"
LABEL org.opencontainers.image.description="Playwright synthetic monitoring runner for FASO DIGITALISATION"
LABEL org.opencontainers.image.licenses="AGPL-3.0-or-later"
LABEL org.opencontainers.image.source="https://github.com/faso-digitalisation/INFRA"

ENV NODE_ENV=production \
    CI=1 \
    PLAYWRIGHT_BROWSERS_PATH=/ms-playwright

WORKDIR /opt/synthetic-monitoring

# Install dependencies first (better layer cache).
COPY package.json ./
RUN npm install --omit=dev --no-audit --no-fund

# Copy test sources and configuration.
COPY playwright.config.ts ./
COPY playwright ./playwright
COPY cron ./cron

# Non-root runtime.
RUN useradd -r -m -u 10001 synthetic \
    && chown -R synthetic:synthetic /opt/synthetic-monitoring
USER synthetic

# Default: run the 5-minute cron loop.
ENTRYPOINT ["/bin/bash", "/opt/synthetic-monitoring/cron/run-all.sh"]
