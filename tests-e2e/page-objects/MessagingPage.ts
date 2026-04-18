import type { Page, Locator } from '@playwright/test';

/**
 * Messagerie. Route: `/messaging` (liste) puis `/messaging/:conversationId`
 * (ChatWindowComponent).
 *
 * Les selecteurs chat utilisent la classe `.thread .msg` et l'input draft
 * `input[name="draft"]`.
 */
export class MessagingPage {
  readonly page: Page;
  readonly heading: Locator;
  readonly conversationsList: Locator;
  readonly messageInput: Locator;
  readonly sendButton: Locator;
  readonly messagesFeed: Locator;
  readonly myMessages: Locator;

  constructor(page: Page) {
    this.page = page;
    this.heading = page.getByRole('heading', { name: /messagerie|conversations/i });
    this.conversationsList = page.locator('a[href*="/messaging/"], .conversation-item');
    this.messageInput = page.locator('input[name="draft"]');
    this.sendButton = page.locator('form.composer button[type="submit"]');
    this.messagesFeed = page.locator('.thread .msg .bubble');
    this.myMessages = page.locator('.msg.mine .bubble');
  }

  async goto(): Promise<void> {
    await this.page.goto('/messaging');
  }

  async openFirstConversation(): Promise<void> {
    const first = this.conversationsList.first();
    await first.waitFor({ state: 'visible', timeout: 5_000 });
    await first.click();
  }

  async sendMessage(text: string): Promise<void> {
    await this.messageInput.fill(text);
    await this.sendButton.click();
  }

  async waitForMessage(text: string, timeoutMs = 10_000): Promise<void> {
    await this.messagesFeed
      .filter({ hasText: text })
      .first()
      .waitFor({ state: 'visible', timeout: timeoutMs });
  }
}
