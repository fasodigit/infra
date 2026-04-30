export const environment = {
  production: false,
  // Full sovereign path: Browser → ARMAGEDDON :8080 → BFF :4800
  // ARMAGEDDON's `/api/auth/*` route now proxies to default-backend (BFF).
  // Kratos is also reachable via `/auth/*` route through the gateway.
  bffUrl: 'http://localhost:8080',
  kratosPublicUrl: 'http://localhost:4433',
  appName: 'Poulets Platform',
  appVersion: '0.1.0',
};
