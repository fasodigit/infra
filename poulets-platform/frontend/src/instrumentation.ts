// SPDX-FileCopyrightText: 2026 FASO DIGITALISATION
// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Browser OTel SDK — instruments fetch/XHR/document-load and forwards
// traces to the OTel Collector at http://localhost:4320 (OTLP/HTTP),
// which fans out to Tempo + Jaeger.
//
// Activation: imported as the FIRST line of main.ts so the global tracer
// provider is registered before Angular bootstrap.

import { WebTracerProvider } from '@opentelemetry/sdk-trace-web';
import { BatchSpanProcessor } from '@opentelemetry/sdk-trace-web';
import { OTLPTraceExporter } from '@opentelemetry/exporter-trace-otlp-http';
import { ZoneContextManager } from '@opentelemetry/context-zone';
import { registerInstrumentations } from '@opentelemetry/instrumentation';
import { FetchInstrumentation } from '@opentelemetry/instrumentation-fetch';
import { XMLHttpRequestInstrumentation } from '@opentelemetry/instrumentation-xml-http-request';
import { DocumentLoadInstrumentation } from '@opentelemetry/instrumentation-document-load';
import { Resource } from '@opentelemetry/resources';
import { SemanticResourceAttributes } from '@opentelemetry/semantic-conventions';

const OTEL_ENDPOINT =
  // Browser bundles cannot read process.env at runtime — Angular embeds
  // environment.ts values at build time. The dev OTel Collector listens on
  // 127.0.0.1:4320; in prod the BFF should proxy /otel-collector → backend.
  (window as unknown as { __FASO_OTEL_ENDPOINT?: string }).__FASO_OTEL_ENDPOINT ??
  'http://localhost:4320';

const provider = new WebTracerProvider({
  resource: new Resource({
    [SemanticResourceAttributes.SERVICE_NAME]: 'poulets-frontend',
    [SemanticResourceAttributes.SERVICE_NAMESPACE]: 'faso',
    [SemanticResourceAttributes.SERVICE_VERSION]: '1.0.0',
    [SemanticResourceAttributes.DEPLOYMENT_ENVIRONMENT]: 'development',
    sovereignty: 'faso-digitalisation',
  }),
});

provider.addSpanProcessor(
  new BatchSpanProcessor(
    new OTLPTraceExporter({
      url: `${OTEL_ENDPOINT}/v1/traces`,
    }),
    {
      // Tighter batching — browser sessions are short-lived.
      maxQueueSize: 100,
      scheduledDelayMillis: 2000,
    },
  ),
);

provider.register({
  // Zone.js context manager: required for Angular Zone-aware tracing so the
  // active span is correctly inherited across async boundaries (RxJS, NgZone).
  contextManager: new ZoneContextManager(),
});

registerInstrumentations({
  instrumentations: [
    new DocumentLoadInstrumentation(),
    new FetchInstrumentation({
      // Propagate W3C traceparent to BFF/auth-ms so spans link.
      propagateTraceHeaderCorsUrls: [
        /localhost:4800.*/, // BFF
        /localhost:8801.*/, // auth-ms (direct calls if any)
        /localhost:8901.*/, // poulets-api (direct calls if any)
      ],
      clearTimingResources: true,
    }),
    new XMLHttpRequestInstrumentation({
      propagateTraceHeaderCorsUrls: [
        /localhost:4800.*/,
        /localhost:8801.*/,
        /localhost:8901.*/,
      ],
    }),
  ],
});
