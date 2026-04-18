import { authenticator } from 'otplib';

authenticator.options = {
  step: 30,
  window: 1,
  digits: 6,
};

export class TotpGen {
  private readonly secret: string;

  constructor(secret: string) {
    this.secret = secret.replace(/\s+/g, '').toUpperCase();
  }

  static fromOtpAuthUri(uri: string): TotpGen {
    const match = uri.match(/[?&]secret=([^&]+)/i);
    if (!match || !match[1]) {
      throw new Error(`Secret introuvable dans l'URI TOTP: ${uri}`);
    }
    return new TotpGen(decodeURIComponent(match[1]));
  }

  static random(): TotpGen {
    return new TotpGen(authenticator.generateSecret());
  }

  code(): string {
    return authenticator.generate(this.secret);
  }

  verify(token: string): boolean {
    return authenticator.verify({ token, secret: this.secret });
  }

  getSecret(): string {
    return this.secret;
  }

  getOtpAuthUri(account: string, issuer = 'FASO'): string {
    return authenticator.keyuri(account, issuer, this.secret);
  }
}
