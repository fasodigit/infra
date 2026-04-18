import { request, type APIRequestContext } from '@playwright/test';

export interface KratosIdentity {
  id: string;
  schema_id?: string;
  traits?: Record<string, unknown>;
  state?: string;
}

export class KratosAdmin {
  private readonly adminURL: string;
  private readonly publicURL: string;

  constructor(
    adminURL: string = process.env.KRATOS_ADMIN_URL ?? 'http://localhost:4434',
    publicURL: string = process.env.KRATOS_PUBLIC_URL ?? 'http://localhost:4433',
  ) {
    this.adminURL = adminURL;
    this.publicURL = publicURL;
  }

  private async api(): Promise<APIRequestContext> {
    return request.newContext();
  }

  async listIdentities(): Promise<KratosIdentity[]> {
    const api = await this.api();
    const res = await api.get(`${this.adminURL}/admin/identities`);
    if (!res.ok()) return [];
    return (await res.json()) as KratosIdentity[];
  }

  async deleteIdentity(id: string): Promise<boolean> {
    const api = await this.api();
    const res = await api.delete(`${this.adminURL}/admin/identities/${id}`);
    return res.ok();
  }

  async wipeAll(): Promise<number> {
    const identities = await this.listIdentities();
    let deleted = 0;
    for (const id of identities) {
      if (await this.deleteIdentity(id.id)) deleted++;
    }
    return deleted;
  }

  async isReachable(): Promise<boolean> {
    try {
      const api = await this.api();
      const res = await api.get(`${this.publicURL}/health/ready`);
      return res.ok();
    } catch {
      return false;
    }
  }

  async getRegistrationFlow(flowId: string): Promise<unknown | null> {
    const api = await this.api();
    const res = await api.get(
      `${this.publicURL}/self-service/registration/flows?id=${encodeURIComponent(flowId)}`,
    );
    if (!res.ok()) return null;
    return res.json();
  }

  async getLoginFlow(flowId: string): Promise<unknown | null> {
    const api = await this.api();
    const res = await api.get(
      `${this.publicURL}/self-service/login/flows?id=${encodeURIComponent(flowId)}`,
    );
    if (!res.ok()) return null;
    return res.json();
  }
}
