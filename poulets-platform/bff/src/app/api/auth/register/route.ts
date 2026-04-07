import { NextRequest, NextResponse } from 'next/server';
import { kratosFrontend, mapKratosSession } from '@/lib/kratos';

/**
 * POST /api/auth/register
 * Registration endpoint that proxies to Kratos native registration flow.
 *
 * Body: { email: string, password: string, name: string, role: string, phone?: string }
 * Response: UserSession on success, error on failure.
 */
export async function POST(request: NextRequest) {
  try {
    const { email, password, name, role, phone } = await request.json();

    if (!email || !password || !name) {
      return NextResponse.json(
        { error: 'Email, password, and name are required' },
        { status: 400 },
      );
    }

    if (role && !['client', 'eleveur'].includes(role)) {
      return NextResponse.json(
        { error: 'Role must be either "client" or "eleveur"' },
        { status: 400 },
      );
    }

    // Step 1: Create a native registration flow
    const { data: flow } = await kratosFrontend.createNativeRegistrationFlow();

    // Step 2: Submit registration data to Kratos
    const { data: regResult, headers: responseHeaders } =
      await kratosFrontend.updateRegistrationFlow({
        flow: flow.id,
        updateRegistrationFlowBody: {
          method: 'password',
          password,
          traits: {
            email,
            name,
            role: role || 'client',
            phone: phone || undefined,
          },
        },
      });

    // Build response
    const session = regResult.session || { identity: regResult.identity };
    const userSession = mapKratosSession(session);
    const response = NextResponse.json(userSession, { status: 201 });

    // Forward Kratos session cookies
    const setCookies = responseHeaders?.['set-cookie'];
    if (setCookies) {
      const cookies = Array.isArray(setCookies) ? setCookies : [setCookies];
      for (const cookie of cookies) {
        response.headers.append('Set-Cookie', cookie);
      }
    }

    return response;
  } catch (error: any) {
    const status = error?.response?.status;
    const kratosData = error?.response?.data;

    if (status === 400) {
      // Check for duplicate email
      const messages = kratosData?.ui?.messages || [];
      const isDuplicate = messages.some(
        (m: any) => m.id === 4000007 || m.text?.includes('already exists'),
      );

      if (isDuplicate) {
        return NextResponse.json(
          { error: 'An account with this email already exists' },
          { status: 409 },
        );
      }

      return NextResponse.json(
        { error: 'Registration validation failed', details: messages },
        { status: 400 },
      );
    }

    console.error('[BFF] Registration error:', error?.message);
    return NextResponse.json(
      { error: 'Registration service unavailable' },
      { status: 503 },
    );
  }
}
