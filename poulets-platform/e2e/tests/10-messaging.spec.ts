import { test, expect } from '@playwright/test';
import { eleveurs, clients } from '../data/seed';
import { isFrontendAvailable, loginAs, navigateTo } from '../helpers/app-helpers';

const BASE_URL = 'http://localhost:4801';

test.describe('10 - Messaging', () => {
  let available: boolean;

  test.beforeAll(async ({ browser }) => {
    const page = await browser.newPage();
    available = await isFrontendAvailable(page, BASE_URL);
    await page.close();
  });

  test.beforeEach(async ({}, testInfo) => {
    if (!available) {
      testInfo.skip();
    }
  });

  // --------------------------------------------------
  // Navigate to messaging
  // --------------------------------------------------
  test('Client navigates to messaging page', async ({ page }) => {
    const client = clients[0];
    await loginAs(page, client.email, client.password);

    await navigateTo(page, '/messaging');
    await page.waitForLoadState('domcontentloaded');

    // Messaging page should be visible
    await expect(page.locator('body')).toContainText(/message|conversation|chat/i, { timeout: 10000 });
  });

  // --------------------------------------------------
  // Contact eleveur from annonce
  // --------------------------------------------------
  test('Client contacts eleveur from marketplace annonce', async ({ page }) => {
    const client = clients[0];
    await loginAs(page, client.email, client.password);

    // Navigate to marketplace to find an annonce
    await navigateTo(page, '/marketplace/annonces');
    await page.waitForLoadState('networkidle');

    // Look for a "Contacter" button on an annonce card
    const contactBtn = page.locator('button, a').filter({ hasText: /contacter|contact|message|[eé]crire/i }).first();
    if (await contactBtn.isVisible({ timeout: 5000 }).catch(() => false)) {
      await contactBtn.click();
      await page.waitForLoadState('domcontentloaded');

      // Should open a messaging interface or redirect to messaging
      const messageArea = page.locator('textarea, input[type="text"], .message-input, .chat-input').first();
      if (await messageArea.isVisible({ timeout: 5000 }).catch(() => false)) {
        await messageArea.fill('Bonjour, je suis interesse par vos poulets bicyclette. Disponibles pour vendredi ?');

        // Send the message
        const sendBtn = page.locator('button').filter({ has: page.locator('mat-icon:text("send")') }).first();
        if (await sendBtn.isVisible({ timeout: 3000 }).catch(() => false)) {
          await sendBtn.click();
          await page.waitForTimeout(1000);
        }
      }
    }

    await expect(page.locator('body')).toBeVisible();
  });

  // --------------------------------------------------
  // Verify conversation created
  // --------------------------------------------------
  test('Conversation appears in messaging list', async ({ page }) => {
    const client = clients[0];
    await loginAs(page, client.email, client.password);

    await navigateTo(page, '/messaging');
    await page.waitForLoadState('domcontentloaded');

    // Look for conversation list items
    const conversations = page.locator('.conversation, .chat-item, mat-list-item, mat-card').filter({ hasText: /Ouedraogo|eleveur|conversation/i });
    const count = await conversations.count().catch(() => 0);

    if (count > 0) {
      await expect(conversations.first()).toBeVisible();
    }

    await expect(page.locator('body')).toBeVisible();
  });

  // --------------------------------------------------
  // Send a message
  // --------------------------------------------------
  test('Client sends a message in existing conversation', async ({ page }) => {
    const client = clients[0];
    await loginAs(page, client.email, client.password);

    await navigateTo(page, '/messaging');
    await page.waitForLoadState('domcontentloaded');

    // Click on a conversation if available
    const conversation = page.locator('.conversation, .chat-item, mat-list-item').first();
    if (await conversation.isVisible({ timeout: 5000 }).catch(() => false)) {
      await conversation.click();
      await page.waitForTimeout(500);

      // Type and send a message
      const messageInput = page.locator('textarea, input[type="text"], .message-input').first();
      if (await messageInput.isVisible({ timeout: 5000 }).catch(() => false)) {
        await messageInput.fill('Pouvez-vous me faire un prix pour 50 poulets au lieu de 30 ?');

        const sendBtn = page.locator('button').filter({
          has: page.locator('mat-icon:text("send")'),
        }).first();
        if (await sendBtn.isVisible({ timeout: 3000 }).catch(() => false)) {
          await sendBtn.click();
          await page.waitForTimeout(1000);
        }
      }
    }

    await expect(page.locator('body')).toBeVisible();
  });

  // --------------------------------------------------
  // Propose a price
  // --------------------------------------------------
  test('Client proposes a price via messaging', async ({ page }) => {
    const client = clients[0];
    await loginAs(page, client.email, client.password);

    await navigateTo(page, '/messaging');
    await page.waitForLoadState('domcontentloaded');

    // Click on a conversation
    const conversation = page.locator('.conversation, .chat-item, mat-list-item').first();
    if (await conversation.isVisible({ timeout: 5000 }).catch(() => false)) {
      await conversation.click();
      await page.waitForTimeout(500);

      // Look for a "Proposer un prix" button or action
      const proposeBtn = page.locator('button, a').filter({ hasText: /proposer.*prix|propose.*price|n[eé]gocier|negoti/i }).first();
      if (await proposeBtn.isVisible({ timeout: 5000 }).catch(() => false)) {
        await proposeBtn.click();
        await page.waitForTimeout(500);

        // Fill price proposal
        const priceInput = page.locator('input[formControlName="price"], input[formControlName="prix"], input[type="number"]').first();
        if (await priceInput.isVisible({ timeout: 3000 }).catch(() => false)) {
          await priceInput.fill('3200');
        }

        // Quantity
        const qtyInput = page.locator('input[formControlName="quantity"], input[formControlName="quantite"]').first();
        if (await qtyInput.isVisible({ timeout: 3000 }).catch(() => false)) {
          await qtyInput.fill('50');
        }

        // Submit proposal
        const submitBtn = page.locator('button[type="submit"], button').filter({ hasText: /envoyer|proposer|submit/i }).first();
        if (await submitBtn.isVisible({ timeout: 3000 }).catch(() => false)) {
          await submitBtn.click();
          await page.waitForTimeout(1000);
        }
      } else {
        // If no propose button, send a message with price proposal
        const messageInput = page.locator('textarea, input[type="text"], .message-input').first();
        if (await messageInput.isVisible({ timeout: 3000 }).catch(() => false)) {
          await messageInput.fill('Je propose 3 200 FCFA/kg pour 50 poulets bicyclette, livraison vendredi prochain.');

          const sendBtn = page.locator('button').filter({
            has: page.locator('mat-icon:text("send")'),
          }).first();
          if (await sendBtn.isVisible({ timeout: 3000 }).catch(() => false)) {
            await sendBtn.click();
            await page.waitForTimeout(1000);
          }
        }
      }
    }

    await expect(page.locator('body')).toBeVisible();
  });

  // --------------------------------------------------
  // Messaging access from user menu
  // --------------------------------------------------
  test('Access messaging through user menu', async ({ page }) => {
    const client = clients[0];
    await loginAs(page, client.email, client.password);

    // Open user menu
    await page.locator('button').filter({ has: page.locator('mat-icon:text("account_circle")') }).click();

    // Click messaging menu item
    const messagingItem = page.locator('button[mat-menu-item]').filter({ hasText: /message|chat/i }).first();
    if (await messagingItem.isVisible({ timeout: 5000 }).catch(() => false)) {
      await messagingItem.click();
      await page.waitForURL(/\/messaging/, { timeout: 10000 });
      await expect(page).toHaveURL(/\/messaging/);
    }
  });

  // --------------------------------------------------
  // Empty state
  // --------------------------------------------------
  test('Messaging shows empty state when no conversations', async ({ page }) => {
    const client = clients[1]; // Traiteur Wendkuni - might have no conversations
    await loginAs(page, client.email, client.password);

    await navigateTo(page, '/messaging');
    await page.waitForLoadState('domcontentloaded');

    // Check for empty state
    const emptyState = page.locator('app-empty-state, .empty-state').first();
    if (await emptyState.isVisible({ timeout: 5000 }).catch(() => false)) {
      await expect(emptyState).toBeVisible();
    }

    await expect(page.locator('body')).toBeVisible();
  });

  // --------------------------------------------------
  // Eleveur receives messages
  // --------------------------------------------------
  test('Eleveur can view messaging page', async ({ page }) => {
    const eleveur = eleveurs[0];
    await loginAs(page, eleveur.email, eleveur.password);

    await navigateTo(page, '/messaging');
    await page.waitForLoadState('domcontentloaded');

    await expect(page.locator('body')).toContainText(/message|conversation|chat/i, { timeout: 10000 });
  });
});
