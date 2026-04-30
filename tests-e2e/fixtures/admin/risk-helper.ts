// SPDX-License-Identifier: AGPL-3.0-or-later
/**
 * RiskHelper — utilitaires pour les specs M14 (risk scoring) :
 *   - SET KAYA `auth:tor:exit_list` avec une IP factice
 *   - SET KAYA `auth:bruteforce:{user}` pour simuler des tentatives
 *   - X-Forwarded-For helpers pour simuler IP source
 *
 * Skip-aware comme `KayaProbe` (terroir).
 */
import { KayaProbe } from '../terroir/kaya-probe';

export interface RiskHelperResult {
  unavailable?: boolean;
  reason?: string;
  applied?: boolean;
}

export class RiskHelper {
  private readonly kaya: KayaProbe;

  constructor(kaya?: KayaProbe) {
    this.kaya = kaya ?? new KayaProbe();
  }

  async isReachable(): Promise<boolean> {
    return this.kaya.isReachable();
  }

  /** Marque une IP comme "Tor exit" pour M14 → +40. */
  async addTorExitIp(ip: string): Promise<RiskHelperResult> {
    const r = await this.kaya.setFlag(`auth:tor:exit_list:${ip}`, '1');
    if (r.unavailable) return { unavailable: true, reason: r.reason };
    return { applied: true };
  }

  async removeTorExitIp(ip: string): Promise<RiskHelperResult> {
    const r = await this.kaya.delFlag(`auth:tor:exit_list:${ip}`);
    if (r.unavailable) return { unavailable: true, reason: r.reason };
    return { applied: true };
  }

  /** Pré-chauffe un device-trust pour M12 (login post-MFA déjà connu). */
  async setDeviceTrust(userId: string, fingerprint: string): Promise<RiskHelperResult> {
    const r = await this.kaya.setFlag(`dev:${userId}:${fingerprint}`, '1');
    if (r.unavailable) return { unavailable: true, reason: r.reason };
    return { applied: true };
  }

  /** Force un compteur de bruteforce pour pousser le score → BLOCK. */
  async setBruteforceCount(userId: string, count: number): Promise<RiskHelperResult> {
    const r = await this.kaya.setFlag(`auth:bruteforce:${userId}`, String(count));
    if (r.unavailable) return { unavailable: true, reason: r.reason };
    return { applied: true };
  }

  /** Headers à envoyer au gateway pour simuler une IP source spécifique. */
  static withSourceIp(ip: string): Record<string, string> {
    return {
      'x-forwarded-for': ip,
      'x-real-ip': ip,
    };
  }

  /** Headers pour simuler un device fingerprint. */
  static withDeviceFingerprint(fingerprint: string): Record<string, string> {
    return {
      'x-device-fp': fingerprint,
      'user-agent': `FasoE2E/${fingerprint.slice(0, 8)}`,
    };
  }
}
