import { request, type APIRequestContext } from '@playwright/test';

interface MailpitSearchMessage {
  ID: string;
  From?: { Address: string; Name?: string };
  To?: Array<{ Address: string; Name?: string }>;
  Subject?: string;
  Created?: string;
}

interface MailpitSearchResponse {
  messages?: MailpitSearchMessage[];
  total?: number;
}

interface MailpitMessageDetail {
  ID: string;
  Text?: string;
  HTML?: string;
  Subject?: string;
}

export interface WaitForOtpOptions {
  regex?: RegExp;
  timeoutMs?: number;
  pollMs?: number;
  deleteAfter?: boolean;
}

export interface WaitForLinkOptions {
  urlRegex?: RegExp;
  timeoutMs?: number;
  pollMs?: number;
  deleteAfter?: boolean;
}

export class MailpitClient {
  private readonly baseURL: string;

  constructor(baseURL?: string) {
    this.baseURL = baseURL ?? process.env.MAILPIT_URL ?? 'http://localhost:8025';
  }

  private async api(): Promise<APIRequestContext> {
    return request.newContext();
  }

  async waitForOtp(email: string, opts: WaitForOtpOptions = {}): Promise<string> {
    const {
      regex = /\b(\d{6})\b/,
      timeoutMs = 15_000,
      pollMs = 200,
      deleteAfter = true,
    } = opts;
    const api = await this.api();
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
      const res = await api.get(
        `${this.baseURL}/api/v1/search?query=${encodeURIComponent(`to:"${email}"`)}`,
      );
      if (res.ok()) {
        const json = (await res.json()) as MailpitSearchResponse;
        const first = json.messages?.[0];
        if (first) {
          const detailRes = await api.get(`${this.baseURL}/api/v1/message/${first.ID}`);
          if (detailRes.ok()) {
            const detail = (await detailRes.json()) as MailpitMessageDetail;
            const body = `${detail.Text ?? ''} ${detail.HTML ?? ''}`;
            const match = body.match(regex);
            if (match && match[1]) {
              if (deleteAfter) {
                await api.delete(`${this.baseURL}/api/v1/messages`, {
                  data: { IDs: [first.ID] },
                });
              }
              return match[1];
            }
          }
        }
      }
      await new Promise((r) => setTimeout(r, pollMs));
    }
    throw new Error(`OTP introuvable pour ${email} (timeout ${timeoutMs}ms)`);
  }

  async waitForLink(email: string, opts: WaitForLinkOptions = {}): Promise<string> {
    const {
      urlRegex = /(https?:\/\/[^\s"'<>]+)/,
      timeoutMs = 15_000,
      pollMs = 200,
      deleteAfter = true,
    } = opts;
    const api = await this.api();
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
      const res = await api.get(
        `${this.baseURL}/api/v1/search?query=${encodeURIComponent(`to:"${email}"`)}`,
      );
      if (res.ok()) {
        const json = (await res.json()) as MailpitSearchResponse;
        const first = json.messages?.[0];
        if (first) {
          const detailRes = await api.get(`${this.baseURL}/api/v1/message/${first.ID}`);
          if (detailRes.ok()) {
            const detail = (await detailRes.json()) as MailpitMessageDetail;
            const body = `${detail.Text ?? ''} ${detail.HTML ?? ''}`;
            const match = body.match(urlRegex);
            if (match && match[1]) {
              if (deleteAfter) {
                await api.delete(`${this.baseURL}/api/v1/messages`, {
                  data: { IDs: [first.ID] },
                });
              }
              return match[1];
            }
          }
        }
      }
      await new Promise((r) => setTimeout(r, pollMs));
    }
    throw new Error(`Lien introuvable pour ${email} (timeout ${timeoutMs}ms)`);
  }

  async clearAll(): Promise<void> {
    const api = await this.api();
    await api.delete(`${this.baseURL}/api/v1/messages`);
  }

  async countForEmail(email: string): Promise<number> {
    const api = await this.api();
    const res = await api.get(
      `${this.baseURL}/api/v1/search?query=${encodeURIComponent(`to:"${email}"`)}`,
    );
    if (!res.ok()) return 0;
    const json = (await res.json()) as MailpitSearchResponse;
    return json.messages?.length ?? 0;
  }

  async isReachable(): Promise<boolean> {
    try {
      const api = await this.api();
      const res = await api.get(`${this.baseURL}/api/v1/info`);
      return res.ok();
    } catch {
      return false;
    }
  }
}
