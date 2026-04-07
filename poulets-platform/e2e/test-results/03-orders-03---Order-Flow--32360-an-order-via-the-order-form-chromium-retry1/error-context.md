# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: 03-orders.spec.ts >> 03 - Order Flow >> Client creates an order via the order form
- Location: tests/03-orders.spec.ts:25:7

# Error details

```
Test timeout of 60000ms exceeded.
```

```
Error: locator.click: Test timeout of 60000ms exceeded.
Call log:
  - waiting for locator('button[matStepperNext]').first()

```

# Page snapshot

```yaml
- generic [ref=e4]:
  - generic [ref=e7]:
    - generic [ref=e9]: Poulets BF
    - separator [ref=e10]
    - generic [ref=e11]: Espace Client
    - navigation [ref=e12]:
      - link "Tableau de bord" [ref=e13] [cursor=pointer]:
        - /url: /dashboard
        - img [ref=e14]: dashboard
        - generic [ref=e15]: Tableau de bord
      - link "Catalogue (Marketplace)" [ref=e16] [cursor=pointer]:
        - /url: /marketplace
        - img [ref=e17]: storefront
        - generic [ref=e18]: Catalogue (Marketplace)
      - link "Publier un besoin" [ref=e19] [cursor=pointer]:
        - /url: /marketplace
        - img [ref=e20]: assignment
        - generic [ref=e21]: Publier un besoin
      - link "Mes commandes" [ref=e22] [cursor=pointer]:
        - /url: /orders
        - img [ref=e23]: shopping_cart
        - generic [ref=e24]: Mes commandes
      - link "Calendrier livraisons" [ref=e25] [cursor=pointer]:
        - /url: /calendar
        - img [ref=e26]: event
        - generic [ref=e27]: Calendrier livraisons
      - link "Contrats" [ref=e28] [cursor=pointer]:
        - /url: /contracts
        - img [ref=e29]: description
        - generic [ref=e30]: Contrats
      - link "Messagerie" [ref=e31] [cursor=pointer]:
        - /url: /messaging
        - img [ref=e32]: chat
        - generic [ref=e33]: Messagerie
      - link "Mon profil" [ref=e34] [cursor=pointer]:
        - /url: /profile
        - img [ref=e35]: person
        - generic [ref=e36]: Mon profil
  - generic [ref=e38]:
    - generic [ref=e39]:
      - button "menu" [ref=e40] [cursor=pointer]:
        - img [ref=e41]: menu
      - link "Poulets BF" [ref=e44] [cursor=pointer]:
        - /url: /dashboard
        - generic [ref=e45]: Poulets BF
      - button [ref=e47] [cursor=pointer]:
        - img [ref=e48]: language
      - button [ref=e51] [cursor=pointer]:
        - img [ref=e52]: notifications
      - button [ref=e55] [cursor=pointer]:
        - img [ref=e56]: account_circle
    - main [ref=e59]
    - contentinfo [ref=e60]: FASO DIGITALISATION - Poulets Platform v0.1.0
```

# Test source

```ts
  1   | import { test, expect } from '@playwright/test';
  2   | import { eleveurs, clients, annonces } from '../data/seed';
  3   | import { isFrontendAvailable, loginAs, navigateTo } from '../helpers/app-helpers';
  4   | 
  5   | const BASE_URL = 'http://localhost:4801';
  6   | 
  7   | test.describe('03 - Order Flow', () => {
  8   |   let available: boolean;
  9   | 
  10  |   test.beforeAll(async ({ browser }) => {
  11  |     const page = await browser.newPage();
  12  |     available = await isFrontendAvailable(page, BASE_URL);
  13  |     await page.close();
  14  |   });
  15  | 
  16  |   test.beforeEach(async ({}, testInfo) => {
  17  |     if (!available) {
  18  |       testInfo.skip();
  19  |     }
  20  |   });
  21  | 
  22  |   // --------------------------------------------------
  23  |   // Client: Create an order
  24  |   // --------------------------------------------------
  25  |   test('Client creates an order via the order form', async ({ page }) => {
  26  |     const client = clients[0];
  27  |     await loginAs(page, client.email, client.password);
  28  | 
  29  |     await navigateTo(page, '/orders/new');
  30  |     await page.waitForLoadState('domcontentloaded');
  31  | 
  32  |     // Step 1: Product selection
  33  |     const raceSelect = page.locator('mat-select[formControlName="race"]').first();
  34  |     if (await raceSelect.isVisible({ timeout: 5000 }).catch(() => false)) {
  35  |       await raceSelect.click({ force: true });
  36  |       await page.locator('mat-option').filter({ hasText: /bicyclette|local/i }).first().click();
  37  |     }
  38  | 
  39  |     // Quantity
  40  |     const qtyInput = page.locator('input[formControlName="quantite"]').first();
  41  |     if (await qtyInput.isVisible({ timeout: 3000 }).catch(() => false)) {
  42  |       await qtyInput.clear();
  43  |       await qtyInput.fill('30');
  44  |     }
  45  | 
  46  |     // Price per unit
  47  |     const priceInput = page.locator('input[formControlName="prixUnitaire"]').first();
  48  |     if (await priceInput.isVisible({ timeout: 3000 }).catch(() => false)) {
  49  |       await priceInput.clear();
  50  |       await priceInput.fill(String(annonces[0].pricePerUnit));
  51  |     }
  52  | 
  53  |     // Verify total is calculated
  54  |     const totalPreview = page.locator('.total-preview .total-value, .total-value').first();
  55  |     if (await totalPreview.isVisible({ timeout: 3000 }).catch(() => false)) {
  56  |       await expect(totalPreview).not.toHaveText('0');
  57  |     }
  58  | 
  59  |     // Next to Step 2: Delivery
> 60  |     await page.locator('button[matStepperNext]').first().click();
      |                                                          ^ Error: locator.click: Test timeout of 60000ms exceeded.
  61  |     await page.waitForTimeout(500);
  62  | 
  63  |     // Delivery date
  64  |     const deliveryDateInput = page.locator('input[formControlName="dateLivraison"]').first();
  65  |     if (await deliveryDateInput.isVisible({ timeout: 5000 }).catch(() => false)) {
  66  |       await deliveryDateInput.fill('2026-05-20');
  67  |     }
  68  | 
  69  |     // Delivery mode - self pickup
  70  |     const selfRadio = page.locator('mat-radio-button[value="self"]').first();
  71  |     if (await selfRadio.isVisible({ timeout: 3000 }).catch(() => false)) {
  72  |       await selfRadio.click();
  73  |     }
  74  | 
  75  |     // Delivery address
  76  |     const addressInput = page.locator('input[formControlName="adresseLivraison"]').first();
  77  |     if (await addressInput.isVisible({ timeout: 3000 }).catch(() => false)) {
  78  |       await addressInput.fill('Ouagadougou, Zone du Bois');
  79  |     }
  80  | 
  81  |     // Phone
  82  |     const phoneInput = page.locator('input[formControlName="telephone"]').first();
  83  |     if (await phoneInput.isVisible({ timeout: 3000 }).catch(() => false)) {
  84  |       await phoneInput.fill('+22625334455');
  85  |     }
  86  | 
  87  |     // Next to Step 3: Payment
  88  |     await page.locator('button[matStepperNext]').nth(1).click();
  89  |     await page.waitForTimeout(500);
  90  | 
  91  |     // Select Orange Money
  92  |     const orangeRadio = page.locator('mat-radio-button[value="orange_money"]').first();
  93  |     if (await orangeRadio.isVisible({ timeout: 5000 }).catch(() => false)) {
  94  |       await orangeRadio.click();
  95  |     }
  96  | 
  97  |     // Notes
  98  |     const notesInput = page.locator('textarea[formControlName="notes"]').first();
  99  |     if (await notesInput.isVisible({ timeout: 3000 }).catch(() => false)) {
  100 |       await notesInput.fill('Livraison vendredi matin SVP');
  101 |     }
  102 | 
  103 |     // Order summary should be visible
  104 |     const summarySection = page.locator('.order-summary');
  105 |     if (await summarySection.isVisible({ timeout: 3000 }).catch(() => false)) {
  106 |       await expect(summarySection).toContainText(/total/i);
  107 |     }
  108 | 
  109 |     // Submit order
  110 |     const confirmBtn = page.locator('button').filter({ hasText: /confirm|commander/i }).first();
  111 |     if (await confirmBtn.isVisible({ timeout: 3000 }).catch(() => false)) {
  112 |       await confirmBtn.click();
  113 |     }
  114 | 
  115 |     // Verify order was submitted (snackbar or redirect)
  116 |     await page.waitForTimeout(2000);
  117 |     await expect(page.locator('body')).toBeVisible();
  118 |   });
  119 | 
  120 |   // --------------------------------------------------
  121 |   // Client: View orders list
  122 |   // --------------------------------------------------
  123 |   test('Client sees order in "Mes commandes"', async ({ page }) => {
  124 |     const client = clients[0];
  125 |     await loginAs(page, client.email, client.password);
  126 | 
  127 |     await navigateTo(page, '/orders');
  128 |     await page.waitForLoadState('networkidle');
  129 | 
  130 |     // The orders page should be visible
  131 |     await expect(page.locator('body')).toContainText(/commande|order/i, { timeout: 10000 });
  132 |   });
  133 | 
  134 |   test('Client order shows status "En attente"', async ({ page }) => {
  135 |     const client = clients[0];
  136 |     await loginAs(page, client.email, client.password);
  137 | 
  138 |     await navigateTo(page, '/orders');
  139 |     await page.waitForLoadState('networkidle');
  140 | 
  141 |     // Look for "En attente" or "En_attente" status in the orders list
  142 |     const statusBadge = page.locator('mat-chip, .status-badge, [class*="status"]').filter({ hasText: /attente|pending/i }).first();
  143 |     if (await statusBadge.isVisible({ timeout: 5000 }).catch(() => false)) {
  144 |       await expect(statusBadge).toBeVisible();
  145 |     }
  146 |   });
  147 | 
  148 |   // --------------------------------------------------
  149 |   // Eleveur: View received orders
  150 |   // --------------------------------------------------
  151 |   test('Eleveur sees orders in "Commandes recues"', async ({ page }) => {
  152 |     const eleveur = eleveurs[0];
  153 |     await loginAs(page, eleveur.email, eleveur.password);
  154 | 
  155 |     await navigateTo(page, '/orders');
  156 |     await page.waitForLoadState('networkidle');
  157 | 
  158 |     // The orders page should load for the eleveur
  159 |     await expect(page.locator('body')).toContainText(/commande|order/i, { timeout: 10000 });
  160 |   });
```