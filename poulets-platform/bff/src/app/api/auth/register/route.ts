import { NextRequest, NextResponse } from 'next/server';
import { mapKratosSession } from '@/lib/kratos';

const KRATOS_PUBLIC_URL = process.env.KRATOS_PUBLIC_URL || 'http://localhost:4433';
const KRATOS_ADMIN_URL = process.env.KRATOS_ADMIN_URL || 'http://localhost:4434';

/**
 * POST /api/auth/register
 * Registration endpoint that proxies to Kratos native registration flow.
 * Uses direct HTTP calls instead of @ory/client for reliability.
 *
 * Body: { email: string, password: string, name: string, role: string, phone?: string }
 * Response: UserSession on success, error on failure.
 */
export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const email = body.email;
    const password = body.password;
    const name = body.name || body.nom; // Angular sends 'nom', tests may send 'name'
    const role = body.role;
    const phone = body.phone;

    if (!email || !password || !name) {
      return NextResponse.json(
        { error: 'Email, password, and name are required' },
        { status: 400 },
      );
    }

    const allowedRoles = ['client', 'eleveur', 'admin', 'operator'];
    const effectiveRole = allowedRoles.includes(role) ? role : 'client';

    // Split name into first_name / last_name for Kratos schema
    const nameParts = (name || '').trim().split(/\s+/);
    const firstName = nameParts[0] || name;
    const lastName = nameParts.length > 1 ? nameParts.slice(1).join(' ') : firstName;

    // Step 1: Create a native registration flow
    const flowRes = await fetch(`${KRATOS_PUBLIC_URL}/self-service/registration/api`, {
      method: 'GET',
    });
    if (!flowRes.ok) {
      console.error('[BFF] Failed to create registration flow:', flowRes.status);
      return NextResponse.json(
        { error: 'Registration service unavailable' },
        { status: 503 },
      );
    }
    const flow = await flowRes.json();

    // Build traits matching the Kratos identity schema
    const traits: Record<string, string> = {
      email,
      first_name: firstName,
      last_name: lastName,
      role: effectiveRole,
    };
    if (phone) {
      traits.phone = phone;
    }

    // Step 2: Submit registration data to Kratos
    const regRes = await fetch(
      `${KRATOS_PUBLIC_URL}/self-service/registration?flow=${flow.id}`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          method: 'password',
          password,
          traits,
        }),
      },
    );

    const regData = await regRes.json();

    if (!regRes.ok) {
      // Collect error messages from nodes
      const nodeMessages = (regData?.ui?.nodes || []).flatMap(
        (n: any) => (n.messages || []).map((m: any) => m.text),
      );
      const topMessages = (regData?.ui?.messages || []).map((m: any) => m.text);
      const allMessages = [...topMessages, ...nodeMessages].filter(Boolean);

      console.error('[BFF] Registration failed:', regRes.status, allMessages);

      const isDuplicate = allMessages.some(
        (t: string) => t.includes('already exists'),
      );
      if (isDuplicate) {
        return NextResponse.json(
          { error: 'An account with this email already exists' },
          { status: 409 },
        );
      }

      return NextResponse.json(
        { error: 'Registration validation failed', details: allMessages },
        { status: 400 },
      );
    }

    // Auto-verify the email address via admin API (DEV convenience)
    const identity = regData.session?.identity || regData.identity;
    if (identity?.id) {
      try {
        await fetch(`${KRATOS_ADMIN_URL}/admin/identities/${identity.id}`, {
          method: 'PATCH',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify([
            { op: 'replace', path: '/verifiable_addresses/0/verified', value: true },
            { op: 'replace', path: '/verifiable_addresses/0/verified_at', value: new Date().toISOString() },
            { op: 'replace', path: '/verifiable_addresses/0/status', value: 'completed' },
          ]),
        });
      } catch (verifyErr: any) {
        console.warn('[BFF] Auto-verify failed (non-fatal):', verifyErr?.message);
      }
    }

    // Build response
    const session = regData.session || { identity: regData.identity };
    const userSession = mapKratosSession(session);
    const response = NextResponse.json(userSession, { status: 201 });

    // Forward session token as cookie if available
    const sessionToken = regData.session_token;
    if (sessionToken) {
      response.cookies.set('ory_kratos_session', sessionToken, {
        httpOnly: true,
        sameSite: 'lax',
        path: '/',
        domain: 'localhost',
      });
    }

    return response;
  } catch (error: any) {
    console.error('[BFF] Registration error:', error?.message);
    return NextResponse.json(
      { error: 'Registration service unavailable' },
      { status: 503 },
    );
  }
}
