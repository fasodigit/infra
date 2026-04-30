// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * PushApprovalHelper — client WebSocket pour le push-approval (M13)
 * exposé par ARMAGEDDON sur `/ws/admin/approval` avec sub-protocol
 * `bearer.<jwt>`.
 *
 * Le `ws` package est en optionalDependencies du tests-e2e — le helper
 * tente un import dynamique et renvoie `{unavailable:true}` quand le
 * paquet ou la stack ne sont pas dispos.
 *
 * Couverture specs : #27 (happy WS), #28 (number-mismatch), #29 (timeout
 * 30s → fallback OTP).
 */
export interface PushWsResult {
  unavailable?: boolean;
  reason?: string;
  connected?: boolean;
  receivedMessages?: unknown[];
}

interface WebSocketLike {
  on(event: 'open', listener: () => void): unknown;
  on(event: 'message', listener: (data: Buffer | ArrayBuffer | string) => void): unknown;
  on(event: 'error', listener: (err: Error) => void): unknown;
  on(event: 'close', listener: (code: number) => void): unknown;
  send(data: string): void;
  close(): void;
  readyState: number;
}

export class PushApprovalHelper {
  private readonly wsURL: string;

  constructor(wsURL?: string) {
    this.wsURL = wsURL ?? process.env.ARMAGEDDON_WS_URL ?? 'ws://localhost:8080/ws/admin/approval';
  }

  private async loadWs(): Promise<{ create: (url: string, sub: string[]) => WebSocketLike } | null> {
    try {
      const path = 'ws';
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const mod: any = await import(/* @vite-ignore */ path);
      const Ctor = mod.WebSocket ?? mod.default ?? mod;
      return {
        create: (url: string, sub: string[]) => new Ctor(url, sub) as WebSocketLike,
      };
    } catch {
      return null;
    }
  }

  /** Tente d'ouvrir une WS et écoute pendant `timeoutMs`. */
  async listen(
    sessionToken: string,
    opts: { timeoutMs?: number; onMessage?: (msg: unknown) => boolean } = {},
  ): Promise<PushWsResult> {
    const factory = await this.loadWs();
    if (!factory) return { unavailable: true, reason: 'ws-driver-missing' };
    const timeout = opts.timeoutMs ?? 5000;
    return new Promise<PushWsResult>((resolve) => {
      let ws: WebSocketLike | null = null;
      const messages: unknown[] = [];
      let connected = false;
      const t = setTimeout(() => {
        try {
          ws?.close();
        } catch {
          // ignore
        }
        resolve({ connected, receivedMessages: messages });
      }, timeout);
      try {
        ws = factory.create(this.wsURL, [`bearer.${sessionToken}`]);
        ws.on('open', () => {
          connected = true;
        });
        ws.on('message', (data) => {
          try {
            const text = typeof data === 'string' ? data : Buffer.from(data as ArrayBuffer).toString();
            const json = JSON.parse(text);
            messages.push(json);
            if (opts.onMessage && opts.onMessage(json)) {
              clearTimeout(t);
              ws?.close();
              resolve({ connected, receivedMessages: messages });
            }
          } catch {
            messages.push(data);
          }
        });
        ws.on('error', (err) => {
          clearTimeout(t);
          resolve({
            unavailable: true,
            reason: `ws-error: ${err.message}`,
            connected,
            receivedMessages: messages,
          });
        });
        ws.on('close', () => {
          clearTimeout(t);
          resolve({ connected, receivedMessages: messages });
        });
      } catch (e) {
        clearTimeout(t);
        resolve({
          unavailable: true,
          reason: e instanceof Error ? e.message : 'ws-unknown',
        });
      }
    });
  }
}
