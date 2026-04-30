// SPDX-FileCopyrightText: 2026 FASO DIGITALISATION
// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Next.js instrumentation hook — initialises the OpenTelemetry Node SDK
// before any request is served. Spans flow to the OTel Collector at
// http://localhost:4320 (OTLP/HTTP) which fans out to Tempo + Jaeger.
//
// Activation: Next.js calls `register()` once at boot when the file is
// at the project root and `experimental.instrumentationHook` is enabled in
// next.config.* (or auto-enabled in Next 15+).

export async function register() {
  if (process.env.NEXT_RUNTIME !== 'nodejs') {
    return;
  }

  const { NodeSDK } = await import('@opentelemetry/sdk-node');
  const { getNodeAutoInstrumentations } = await import(
    '@opentelemetry/auto-instrumentations-node'
  );
  const { OTLPTraceExporter } = await import(
    '@opentelemetry/exporter-trace-otlp-http'
  );
  const { Resource } = await import('@opentelemetry/resources');
  const { SemanticResourceAttributes } = await import(
    '@opentelemetry/semantic-conventions'
  );

  const otlpEndpoint =
    process.env.OTEL_EXPORTER_OTLP_ENDPOINT ?? 'http://localhost:4320';

  const sdk = new NodeSDK({
    resource: new Resource({
      [SemanticResourceAttributes.SERVICE_NAME]: 'poulets-bff',
      [SemanticResourceAttributes.SERVICE_NAMESPACE]: 'faso',
      [SemanticResourceAttributes.SERVICE_VERSION]:
        process.env.npm_package_version ?? '1.0.0',
      [SemanticResourceAttributes.DEPLOYMENT_ENVIRONMENT]:
        process.env.FASO_ENV ?? 'development',
      sovereignty: 'faso-digitalisation',
    }),
    traceExporter: new OTLPTraceExporter({
      url: `${otlpEndpoint}/v1/traces`,
    }),
    instrumentations: [
      getNodeAutoInstrumentations({
        // Disable noisy/expensive instrumentations on the dev BFF.
        '@opentelemetry/instrumentation-fs': { enabled: false },
        '@opentelemetry/instrumentation-net': { enabled: false },
        '@opentelemetry/instrumentation-dns': { enabled: false },
        // Forward incoming request headers as span attributes (helpful for
        // correlating BFF spans with the upstream Angular trace).
        '@opentelemetry/instrumentation-http': {
          requestHook: (span, request) => {
            const traceparent =
              (request as { headers?: Record<string, unknown> }).headers?.[
                'traceparent'
              ];
            if (typeof traceparent === 'string') {
              span.setAttribute('http.request.traceparent', traceparent);
            }
          },
        },
      }),
    ],
  });

  sdk.start();

  // Graceful shutdown handler removed — Next.js static analyzer flags
  // `process.on` as Edge-incompatible even behind a runtime guard, which
  // breaks compilation of API route handlers that share the bundle. The
  // OTel BatchSpanProcessor flushes on process exit anyway; explicit
  // shutdown is only needed in custom Node entry-points.
}
