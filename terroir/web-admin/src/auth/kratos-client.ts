// SPDX-License-Identifier: AGPL-3.0-or-later
//
// ORY Kratos public flow client (proxied via ARMAGEDDON :8080).
// L'admin-UI consomme uniquement /auth/* (jamais :4433 en direct).

export interface KratosFlow {
  id: string;
  type: string;
  ui: {
    action: string;
    method: string;
    nodes: KratosNode[];
  };
  expires_at?: string;
}

export interface KratosNode {
  type: string;
  group: string;
  attributes: {
    name: string;
    type: string;
    value?: string;
    required?: boolean;
  };
  messages?: Array<{ id: number; text: string; type: string }>;
}

export interface KratosSession {
  id: string;
  active: boolean;
  identity: {
    id: string;
    traits: {
      email: string;
      name?: { first?: string; last?: string };
    };
    metadata_public?: {
      role?: string;
      cooperative_id?: string;
      union_id?: string;
    };
  };
}

const AUTH_BASE = '/auth';

export async function initLoginFlow(): Promise<KratosFlow> {
  const res = await fetch(`${AUTH_BASE}/self-service/login/browser`, {
    credentials: 'include',
    headers: { Accept: 'application/json' },
  });
  if (!res.ok) {
    throw new Error(`Kratos init flow failed: ${res.status}`);
  }
  return (await res.json()) as KratosFlow;
}

export async function submitLogin(
  flow: KratosFlow,
  email: string,
  password: string,
): Promise<KratosSession> {
  const csrfNode = flow.ui.nodes.find(
    (n) => n.attributes.name === 'csrf_token',
  );
  const body = {
    method: 'password',
    identifier: email,
    password,
    csrf_token: csrfNode?.attributes.value ?? '',
  };
  const res = await fetch(flow.ui.action, {
    method: 'POST',
    credentials: 'include',
    headers: {
      'Content-Type': 'application/json',
      Accept: 'application/json',
    },
    body: JSON.stringify(body),
  });
  if (!res.ok) {
    const errBody = await res.text();
    throw new Error(`Kratos login failed (${res.status}): ${errBody}`);
  }
  const data = await res.json();
  return data.session as KratosSession;
}

export async function whoami(): Promise<KratosSession | null> {
  const res = await fetch(`${AUTH_BASE}/whoami`, {
    credentials: 'include',
    headers: { Accept: 'application/json' },
  });
  if (res.status === 401) return null;
  if (!res.ok) {
    throw new Error(`whoami failed: ${res.status}`);
  }
  return (await res.json()) as KratosSession;
}

export async function logout(): Promise<void> {
  const res = await fetch(`${AUTH_BASE}/self-service/logout/browser`, {
    credentials: 'include',
  });
  if (!res.ok) {
    throw new Error(`logout failed: ${res.status}`);
  }
  const flow = await res.json();
  if (flow.logout_url) {
    await fetch(flow.logout_url, { credentials: 'include' });
  }
}
