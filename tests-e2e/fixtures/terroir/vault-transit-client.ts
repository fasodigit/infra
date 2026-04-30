// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * VaultTransitClient — wrapper du moteur Transit Vault HashiCorp.
 *
 * Couvre l'usage P0.B `INFRA/terroir/docs/ULTRAPLAN-TERROIR-2026-04-30.md`
 * §4 P0.2 :
 *   - POST /v1/transit/encrypt/terroir-pii-master  (avec context — `derived=true`)
 *   - POST /v1/transit/decrypt/terroir-pii-master
 *   - GET  /v1/transit/keys/terroir-pii-master     (métadonnées rotation)
 *   - GET  /v1/sys/health
 *
 * La clé `terroir-pii-master` est créée par les scripts Vault avec
 * `type=aes256-gcm96`, `derived=true`, `auto_rotate_period=2160h` (90 jours).
 * On NE charge JAMAIS la valeur en clair côté client : tous les ciphertext
 * sont opaques (`vault:v1:...`).
 *
 * Pas de mocks : Vault tourne sur :8200 (cf. CLAUDE.md §2).
 */
import { request, type APIRequestContext } from '@playwright/test';

export interface VaultEncryptRequest {
  /** Plaintext clair. Sera encodé base64 avant envoi. */
  plaintext: string;
  /**
   * Context dérivation. Sera encodé base64. Doit être identique pour
   * encrypt et decrypt sinon la dérivation HKDF échoue.
   */
  context: string;
  keyVersion?: number;
}

export interface VaultEncryptResponse {
  data: {
    ciphertext: string;
    key_version: number;
  };
}

export interface VaultDecryptResponse {
  data: {
    /** Plaintext renvoyé en base64 par Vault — la classe le décode. */
    plaintext: string;
  };
}

export interface VaultKeyInfo {
  data: {
    name: string;
    type: string;
    derived: boolean;
    keys: Record<string, number>;
    min_decryption_version: number;
    min_encryption_version: number;
    latest_version: number;
    auto_rotate_period: string;
    deletion_allowed: boolean;
    exportable: boolean;
  };
}

export interface VaultErrorBody {
  errors: string[];
}

const KEY_NAME = 'terroir-pii-master';

function toB64(s: string): string {
  return Buffer.from(s, 'utf8').toString('base64');
}

function fromB64(s: string): string {
  return Buffer.from(s, 'base64').toString('utf8');
}

export class VaultTransitClient {
  private readonly baseURL: string;
  private readonly token: string;
  private readonly keyName: string;

  constructor(opts?: { baseURL?: string; token?: string; keyName?: string }) {
    this.baseURL = opts?.baseURL ?? process.env.VAULT_ADDR ?? 'http://localhost:8200';
    this.token = opts?.token ?? process.env.VAULT_TOKEN ?? '';
    this.keyName = opts?.keyName ?? KEY_NAME;
    if (!this.token) {
      // Soft warning : les specs vérifieront isReachable() avant d'agir.
      // eslint-disable-next-line no-console
      console.warn('[VaultTransitClient] VAULT_TOKEN absent — calls will 403');
    }
  }

  private async api(): Promise<APIRequestContext> {
    return request.newContext({
      extraHTTPHeaders: {
        'X-Vault-Token': this.token,
        accept: 'application/json',
      },
    });
  }

  /**
   * Encrypt avec context obligatoire (key derived=true). Le ciphertext
   * commence toujours par `vault:v1:` (préfixe Vault Transit standard).
   */
  async encrypt(req: VaultEncryptRequest): Promise<VaultEncryptResponse['data']> {
    const api = await this.api();
    const res = await api.post(
      `${this.baseURL}/v1/transit/encrypt/${this.keyName}`,
      {
        data: {
          plaintext: toB64(req.plaintext),
          context: toB64(req.context),
          ...(req.keyVersion !== undefined ? { key_version: req.keyVersion } : {}),
        },
      },
    );
    if (!res.ok()) {
      throw new Error(
        `Vault encrypt HTTP ${res.status()} : ${await res.text()}`,
      );
    }
    return ((await res.json()) as VaultEncryptResponse).data;
  }

  /**
   * Variante "soft" qui ne throw pas — utile pour tester les error paths
   * (encrypt sans context, decrypt avec ciphertext altéré, etc.).
   */
  async encryptRaw(payload: Record<string, unknown>): Promise<{ status: number; body: unknown }> {
    const api = await this.api();
    const res = await api.post(
      `${this.baseURL}/v1/transit/encrypt/${this.keyName}`,
      { data: payload },
    );
    let body: unknown;
    try {
      body = await res.json();
    } catch {
      body = await res.text();
    }
    return { status: res.status(), body };
  }

  /**
   * Decrypt avec le MÊME context que pour l'encrypt. Plaintext renvoyé
   * en clair (la classe décode le base64 Vault).
   */
  async decrypt(ciphertext: string, context: string): Promise<string> {
    const api = await this.api();
    const res = await api.post(
      `${this.baseURL}/v1/transit/decrypt/${this.keyName}`,
      {
        data: {
          ciphertext,
          context: toB64(context),
        },
      },
    );
    if (!res.ok()) {
      throw new Error(
        `Vault decrypt HTTP ${res.status()} : ${await res.text()}`,
      );
    }
    const json = (await res.json()) as VaultDecryptResponse;
    return fromB64(json.data.plaintext);
  }

  async decryptRaw(payload: Record<string, unknown>): Promise<{ status: number; body: unknown }> {
    const api = await this.api();
    const res = await api.post(
      `${this.baseURL}/v1/transit/decrypt/${this.keyName}`,
      { data: payload },
    );
    let body: unknown;
    try {
      body = await res.json();
    } catch {
      body = await res.text();
    }
    return { status: res.status(), body };
  }

  /** Lit les métadonnées de la clé (versions, rotation, etc.). */
  async getKeyInfo(): Promise<VaultKeyInfo['data']> {
    const api = await this.api();
    const res = await api.get(
      `${this.baseURL}/v1/transit/keys/${this.keyName}`,
    );
    if (!res.ok()) {
      throw new Error(
        `Vault key info HTTP ${res.status()} : ${await res.text()}`,
      );
    }
    return ((await res.json()) as VaultKeyInfo).data;
  }

  async health(): Promise<{ status: number; initialized: boolean; sealed: boolean }> {
    const api = await this.api();
    const res = await api.get(`${this.baseURL}/v1/sys/health`);
    let body: { initialized?: boolean; sealed?: boolean } = {};
    try {
      body = (await res.json()) as { initialized?: boolean; sealed?: boolean };
    } catch {
      // sys/health peut retourner 503 sans body JSON valide
    }
    return {
      status: res.status(),
      initialized: body.initialized ?? false,
      sealed: body.sealed ?? true,
    };
  }

  async isReachable(): Promise<boolean> {
    try {
      const h = await this.health();
      // 200 = init + unsealed + active ; 429 = standby (acceptable)
      return h.status === 200 || h.status === 429;
    } catch {
      return false;
    }
  }
}
