/** @type {import('next').NextConfig} */
const nextConfig = {
  // Proxy GraphQL requests to the poulets-api backend
  async rewrites() {
    return [
      {
        source: '/api/graphql',
        destination: `${process.env.POULETS_API_GRAPHQL || 'http://localhost:8901/graphql'}`,
      },
    ];
  },

  // Security headers
  async headers() {
    return [
      {
        source: '/api/:path*',
        headers: [
          { key: 'X-Content-Type-Options', value: 'nosniff' },
          { key: 'X-Frame-Options', value: 'DENY' },
          { key: 'X-XSS-Protection', value: '1; mode=block' },
          { key: 'Referrer-Policy', value: 'strict-origin-when-cross-origin' },
        ],
      },
    ];
  },

  // CORS for Angular frontend
  async redirects() {
    return [];
  },

  // Standalone output for Docker
  output: 'standalone',

  // Environment variables exposed to the server
  env: {
    KRATOS_PUBLIC_URL: process.env.KRATOS_PUBLIC_URL || 'http://localhost:4433',
    KRATOS_ADMIN_URL: process.env.KRATOS_ADMIN_URL || 'http://localhost:4434',
    POULETS_API_URL: process.env.POULETS_API_URL || 'http://localhost:8901',
    AUTH_MS_URL: process.env.AUTH_MS_URL || 'http://localhost:8801',
    FRONTEND_URL: process.env.FRONTEND_URL || 'http://localhost:4801',
  },
};

module.exports = nextConfig;
