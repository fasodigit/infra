import type { Page, CDPSession } from '@playwright/test';

export interface VirtualAuthenticatorOptions {
  protocol?: 'u2f' | 'ctap2';
  transport?: 'usb' | 'nfc' | 'ble' | 'internal';
  hasResidentKey?: boolean;
  hasUserVerification?: boolean;
  isUserVerified?: boolean;
  automaticPresenceSimulation?: boolean;
}

export interface VirtualAuthenticator {
  authenticatorId: string;
  cdp: CDPSession;
  remove: () => Promise<void>;
  getCredentials: () => Promise<unknown>;
  clearCredentials: () => Promise<void>;
}

const DEFAULT_OPTIONS: Required<VirtualAuthenticatorOptions> = {
  protocol: 'ctap2',
  transport: 'internal',
  hasResidentKey: true,
  hasUserVerification: true,
  isUserVerified: true,
  automaticPresenceSimulation: true,
};

export async function addVirtualAuthenticator(
  page: Page,
  opts: VirtualAuthenticatorOptions = {},
): Promise<VirtualAuthenticator> {
  const options = { ...DEFAULT_OPTIONS, ...opts };
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('WebAuthn.enable');
  const { authenticatorId } = (await cdp.send('WebAuthn.addVirtualAuthenticator', {
    options: {
      protocol: options.protocol,
      transport: options.transport,
      hasResidentKey: options.hasResidentKey,
      hasUserVerification: options.hasUserVerification,
      isUserVerified: options.isUserVerified,
      automaticPresenceSimulation: options.automaticPresenceSimulation,
    },
  })) as { authenticatorId: string };

  return {
    authenticatorId,
    cdp,
    remove: async () => {
      await cdp.send('WebAuthn.removeVirtualAuthenticator', { authenticatorId });
      await cdp.detach();
    },
    getCredentials: async () => {
      return cdp.send('WebAuthn.getCredentials', { authenticatorId });
    },
    clearCredentials: async () => {
      await cdp.send('WebAuthn.clearCredentials', { authenticatorId });
    },
  };
}
