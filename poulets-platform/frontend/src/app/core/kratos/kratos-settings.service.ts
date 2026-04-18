// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { Injectable, inject, PLATFORM_ID } from '@angular/core';
import { isPlatformBrowser } from '@angular/common';
import { HttpClient } from '@angular/common/http';
import { Observable, of, throwError } from 'rxjs';
import { map, catchError } from 'rxjs/operators';
import { environment } from '@env/environment';
import {
  KratosSettingsFlow, KratosSession, MfaStatus, PasskeyDevice, BackupCodesConfig,
} from './kratos.models';

/**
 * Wrapper autour de `/self-service/settings/*` Kratos.
 *
 * Adapté de Etat-civil actor-ui. Notes importantes :
 * - Les flows Kratos retournent `ui.nodes[]` à rendre dynamiquement. Nous parsons
 *   ces nodes et exposons une API de haut niveau centrée sur l'UX MFA.
 * - En dev, tout est stubé côté client (mocks) tant que Kratos n'est pas activé
 *   avec `webauthn.enabled: true`, `totp.enabled: true`, `lookup_secret.enabled: true`.
 */
@Injectable({ providedIn: 'root' })
export class KratosSettingsService {
  private readonly http = inject(HttpClient);
  private readonly platformId = inject(PLATFORM_ID);
  private readonly kratosUrl = (environment as any).kratosPublicUrl ?? 'http://localhost:4433';

  /** True si on tourne côté navigateur (gating SSR). */
  get isBrowser(): boolean { return isPlatformBrowser(this.platformId); }

  /** Lit la session courante. Nécessaire pour connaître `authenticator_assurance_level`. */
  whoami(): Observable<KratosSession | null> {
    if (!this.isBrowser) return of(null);
    return this.http.get<KratosSession>(`${this.kratosUrl}/sessions/whoami`, {
      withCredentials: true,
      headers: { Accept: 'application/json' },
    }).pipe(catchError(() => of(null)));
  }

  /** Initialise un flow settings (obligatoire avant toute modification). */
  initFlow(): Observable<KratosSettingsFlow> {
    return this.http.get<KratosSettingsFlow>(
      `${this.kratosUrl}/self-service/settings/browser`,
      { withCredentials: true, headers: { Accept: 'application/json' } },
    );
  }

  /** Soumet le body du flow (e.g. totp_code, webauthn_register, lookup_secret_confirm). */
  submit(flowId: string, body: Record<string, unknown>): Observable<KratosSettingsFlow> {
    return this.http.post<KratosSettingsFlow>(
      `${this.kratosUrl}/self-service/settings?flow=${flowId}`,
      body,
      { withCredentials: true, headers: { Accept: 'application/json' } },
    );
  }

  /**
   * Extrait un snapshot MFA consolidé à partir du flow settings courant.
   * En dev, retourne un stub si Kratos n'est pas joignable.
   *
   * IMPORTANT : tant que le BFF ne sait pas routing /self-service/settings avec
   * la session cookie Kratos, on tombe en fallback immédiatement pour permettre
   * à l'UX MFA de s'afficher (tests E2E, dev local sans Kratos fully wired).
   */
  getMfaStatus(): Observable<MfaStatus> {
    if (!this.isBrowser) return of(this.fallbackMfaStatus());
    return this.initFlow().pipe(
      map((flow) => this.parseMfaStatus(flow)),
      catchError(() => of(this.fallbackMfaStatus())),
    );
  }

  private parseMfaStatus(flow: KratosSettingsFlow): MfaStatus {
    const traits = flow.identity.traits as Record<string, any>;
    const va = flow.identity.verifiable_addresses ?? [];
    const creds = flow.identity.credentials ?? {};

    const email = va.find((v) => v.via === 'email');
    const webauthn = creds['webauthn'];
    const totp = creds['totp'];
    const lookup = creds['lookup_secret'];

    const passkeyDevices: PasskeyDevice[] = webauthn?.config?.['credentials']
      ? (webauthn.config['credentials'] as any[]).map((c: any) => ({
          id: c.id,
          name: c.display_name ?? 'Clé sans nom',
          addedAt: c.added_at ?? new Date().toISOString(),
          lastUsedAt: c.last_used_at,
          kind: c.is_platform ? 'platform' : 'cross-platform',
        }))
      : [];

    const remaining = (lookup?.config?.['recovery_codes'] as unknown[] | undefined)?.length ?? 0;

    const status: MfaStatus = {
      email: { verified: email?.verified ?? false, address: email?.value ?? (traits['email'] ?? '') },
      passkey: { configured: passkeyDevices.length > 0, devices: passkeyDevices },
      totp: { configured: !!totp, configuredAt: (totp?.config as any)?.configured_at },
      backupCodes: { generated: !!lookup, remaining },
      phone: { configured: !!traits['phone'], number: traits['phone'] as string | undefined },
      completed: !!traits['mfa_onboarding_completed'],
    };
    return status;
  }

  private fallbackMfaStatus(): MfaStatus {
    // Stub dev when Kratos unreachable. Shows UX without backend.
    return {
      email: { verified: true, address: 'user@example.bf' },
      passkey: { configured: false, devices: [] },
      totp: { configured: false },
      backupCodes: { generated: false, remaining: 0 },
      phone: { configured: false },
      completed: false,
    };
  }

  // --------------------------------------------------------- Mock MFA actions

  /**
   * Stub : génère 10 backup codes côté client.
   * En prod : soumettre `lookup_secret_regenerate` + `lookup_secret_confirm` au flow settings.
   */
  generateBackupCodes(): Observable<string[]> {
    const alphabet = 'ABCDEFGHJKLMNPQRSTUVWXYZ23456789';
    const codes = Array.from({ length: 10 }, () => {
      const part = (n: number) => Array.from({ length: n }, () => alphabet[Math.floor(Math.random() * alphabet.length)]).join('');
      return `${part(4)}-${part(4)}`;
    });
    return of(codes);
  }

  /**
   * Stub : génère une clé TOTP + otpauth URL + secret base32.
   * En prod : l'`totp_qr` node Kratos renvoie déjà le SVG + secret.
   */
  initTotp(issuer: string, account: string): Observable<{ secret: string; otpauth: string }> {
    const secret = this.randomBase32(32);
    const otpauth = `otpauth://totp/${encodeURIComponent(issuer)}:${encodeURIComponent(account)}?secret=${secret}&issuer=${encodeURIComponent(issuer)}&algorithm=SHA1&digits=6&period=30`;
    return of({ secret, otpauth });
  }

  /** Stub verify TOTP. En prod : soumettre `totp_code` au flow. */
  verifyTotp(_code: string): Observable<boolean> {
    // Accept any 6-digit code in dev mock.
    return of(_code.length === 6 && /^\d{6}$/.test(_code));
  }

  private randomBase32(len: number): string {
    const alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567';
    let out = '';
    for (let i = 0; i < len; i++) out += alphabet[Math.floor(Math.random() * alphabet.length)];
    return out;
  }
}
