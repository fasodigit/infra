import { NextResponse } from 'next/server';

/**
 * GET /api/health
 * Health check endpoint for Docker and load balancers.
 */
export async function GET() {
  return NextResponse.json({
    status: 'ok',
    service: 'poulets-bff',
    version: '0.1.0',
    timestamp: new Date().toISOString(),
  });
}
