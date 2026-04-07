# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: 06-contracts.spec.ts >> 06 - Recurring Contracts >> Eleveur creates a new recurring contract via stepper
- Location: tests/06-contracts.spec.ts:39:7

# Error details

```
Test timeout of 60000ms exceeded.
```

```
Error: locator.click: Test timeout of 60000ms exceeded.
Call log:
  - waiting for locator('button[matStepperNext], button').filter({ hasText: /suivant|next/i }).first()
    - locator resolved to <button type="submit" color="primary" disabled="true" matsteppernext="" mat-raised-button="" _ngcontent-ng-c2252583720="" mat-ripple-loader-disabled="" mat-ripple-loader-uninitialized="" mat-ripple-loader-class-name="mat-mdc-button-ripple" class="mdc-button mat-mdc-button-base mat-stepper-next mdc-button--raised mat-mdc-raised-button mat-primary mat-mdc-button-disabled">…</button>
  - attempting click action
    2 × waiting for element to be visible, enabled and stable
      - element is not enabled
    - retrying click action
    - waiting 20ms
    2 × waiting for element to be visible, enabled and stable
      - element is not enabled
    - retrying click action
      - waiting 100ms
    108 × waiting for element to be visible, enabled and stable
        - element is not enabled
      - retrying click action
        - waiting 500ms

```

# Page snapshot

```yaml
- generic [ref=e4]:
  - generic [ref=e7]:
    - generic [ref=e9]: Poulets BF
    - separator [ref=e10]
    - generic [ref=e11]: Espace Éleveur
    - navigation [ref=e12]:
      - link "Tableau de bord" [ref=e13] [cursor=pointer]:
        - /url: /dashboard
        - img [ref=e14]: dashboard
        - generic [ref=e15]: Tableau de bord
      - link "Mes Annonces" [ref=e16] [cursor=pointer]:
        - /url: /marketplace
        - img [ref=e17]: storefront
        - generic [ref=e18]: Mes Annonces
      - link "Mes Lots (Suivi croissance)" [ref=e19] [cursor=pointer]:
        - /url: /growth
        - img [ref=e20]: inventory_2
        - generic [ref=e21]: Mes Lots (Suivi croissance)
      - link "Commandes" [ref=e22] [cursor=pointer]:
        - /url: /orders
        - img [ref=e23]: shopping_cart
        - generic [ref=e24]: Commandes
      - link "Suivi vétérinaire" [ref=e25] [cursor=pointer]:
        - /url: /veterinary
        - img [ref=e26]: medical_services
        - generic [ref=e27]: Suivi vétérinaire
      - link "Certification halal" [ref=e28] [cursor=pointer]:
        - /url: /halal
        - img [ref=e29]: verified
        - generic [ref=e30]: Certification halal
      - link "Contrats" [ref=e31] [cursor=pointer]:
        - /url: /contracts
        - img [ref=e32]: description
        - generic [ref=e33]: Contrats
      - link "Messagerie" [ref=e34] [cursor=pointer]:
        - /url: /messaging
        - img [ref=e35]: chat
        - generic [ref=e36]: Messagerie
      - link "Mon profil" [ref=e37] [cursor=pointer]:
        - /url: /profile
        - img [ref=e38]: person
        - generic [ref=e39]: Mon profil
  - generic [ref=e41]:
    - generic [ref=e42]:
      - button "menu" [ref=e43] [cursor=pointer]:
        - img [ref=e44]: menu
      - link "Poulets BF" [ref=e47] [cursor=pointer]:
        - /url: /dashboard
        - generic [ref=e48]: Poulets BF
      - button [ref=e50] [cursor=pointer]:
        - img [ref=e51]: language
      - button [ref=e54] [cursor=pointer]:
        - img [ref=e55]: notifications
      - button [ref=e58] [cursor=pointer]:
        - img [ref=e59]: account_circle
    - main [ref=e62]:
      - generic [ref=e64]:
        - heading "contracts.create.title" [level=1] [ref=e66]:
          - img [ref=e67]: add_circle
          - text: contracts.create.title
        - generic [ref=e69]:
          - tablist [ref=e70]:
            - tab "contracts.create.step1" [selected] [ref=e71] [cursor=pointer]:
              - generic [ref=e74]: "1"
              - generic [ref=e76]: contracts.create.step1
            - tab "contracts.create.step2" [disabled] [ref=e78] [cursor=pointer]:
              - generic [ref=e81]: "2"
              - generic [ref=e83]: contracts.create.step2
            - tab "contracts.create.step3" [disabled] [ref=e85] [cursor=pointer]:
              - generic [ref=e88]: "3"
              - generic [ref=e90]: contracts.create.step3
            - tab "contracts.create.step4" [disabled] [ref=e92] [cursor=pointer]:
              - generic [ref=e95]: "4"
              - generic [ref=e97]: contracts.create.step4
            - tab "contracts.create.step5" [disabled] [ref=e99] [cursor=pointer]:
              - generic [ref=e102]: "5"
              - generic [ref=e104]: contracts.create.step5
          - tabpanel "contracts.create.step1" [ref=e106]:
            - generic [ref=e108]:
              - heading "contracts.create.selectPartner" [level=3] [ref=e109]
              - generic [ref=e113]:
                - generic [ref=e114]: contracts.create.searchPartner
                - img [ref=e116]: search
                - combobox "contracts.create.searchPartner" [ref=e118]
              - generic [ref=e120]:
                - button "contracts.create.next" [disabled]:
                  - img: arrow_forward
                  - generic: contracts.create.next
    - contentinfo [ref=e121]: FASO DIGITALISATION - Poulets Platform v0.1.0
```

# Test source

```ts
  2   | import { eleveurs, clients, contratRecurrent } from '../data/seed';
  3   | import { isFrontendAvailable, loginAs, navigateTo } from '../helpers/app-helpers';
  4   | 
  5   | const BASE_URL = 'http://localhost:4801';
  6   | 
  7   | test.describe('06 - Recurring Contracts', () => {
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
  23  |   // Navigate to contracts
  24  |   // --------------------------------------------------
  25  |   test('Eleveur navigates to contracts page', async ({ page }) => {
  26  |     const eleveur = eleveurs[0];
  27  |     await loginAs(page, eleveur.email, eleveur.password);
  28  | 
  29  |     await navigateTo(page, '/contracts');
  30  |     await page.waitForLoadState('domcontentloaded');
  31  | 
  32  |     // Contracts page should be visible
  33  |     await expect(page.locator('body')).toContainText(/contrat|contract/i, { timeout: 10000 });
  34  |   });
  35  | 
  36  |   // --------------------------------------------------
  37  |   // Create new contract
  38  |   // --------------------------------------------------
  39  |   test('Eleveur creates a new recurring contract via stepper', async ({ page }) => {
  40  |     const eleveur = eleveurs[0];
  41  |     await loginAs(page, eleveur.email, eleveur.password);
  42  | 
  43  |     await navigateTo(page, '/contracts/new');
  44  |     await page.waitForLoadState('domcontentloaded');
  45  | 
  46  |     const c = contratRecurrent;
  47  | 
  48  |     // The create contract page might have a multi-step form (stepper)
  49  |     // Step 1: Product info
  50  |     const raceSelect = page.locator('mat-select[formControlName="race"]').first();
  51  |     if (await raceSelect.isVisible({ timeout: 5000 }).catch(() => false)) {
  52  |       await raceSelect.click({ force: true });
  53  |       await page.locator('mat-option').filter({ hasText: new RegExp(c.race, 'i') }).first().click();
  54  |     }
  55  | 
  56  |     const qtyInput = page.locator('input[formControlName="quantity"], input[formControlName="quantite"]').first();
  57  |     if (await qtyInput.isVisible({ timeout: 3000 }).catch(() => false)) {
  58  |       await qtyInput.fill(String(c.quantity));
  59  |     }
  60  | 
  61  |     const minWeightInput = page.locator('input[formControlName="minWeight"], input[formControlName="poidsMinimum"]').first();
  62  |     if (await minWeightInput.isVisible({ timeout: 3000 }).catch(() => false)) {
  63  |       await minWeightInput.fill(String(c.minWeight));
  64  |     }
  65  | 
  66  |     const priceInput = page.locator('input[formControlName="pricePerKg"], input[formControlName="prixKg"]').first();
  67  |     if (await priceInput.isVisible({ timeout: 3000 }).catch(() => false)) {
  68  |       await priceInput.fill(String(c.pricePerKg));
  69  |     }
  70  | 
  71  |     // Try to advance to next step (skip if button is disabled due to missing backend data)
  72  |     const nextBtn = page.locator('button[matStepperNext], button').filter({ hasText: /suivant|next/i }).first();
  73  |     if (await nextBtn.isVisible({ timeout: 3000 }).catch(() => false)) {
  74  |       const isDisabled = await nextBtn.isDisabled().catch(() => true);
  75  |       if (!isDisabled) {
  76  |         await nextBtn.click();
  77  |         await page.waitForTimeout(500);
  78  |       }
  79  |     }
  80  | 
  81  |     // Step 2: Frequency
  82  |     const freqSelect = page.locator('mat-select[formControlName="frequency"], mat-select[formControlName="frequence"]').first();
  83  |     if (await freqSelect.isVisible({ timeout: 5000 }).catch(() => false)) {
  84  |       await freqSelect.click({ force: true });
  85  |       await page.locator('mat-option').filter({ hasText: /hebdomadaire|weekly/i }).first().click();
  86  |     }
  87  | 
  88  |     const daySelect = page.locator('mat-select[formControlName="dayPreference"], mat-select[formControlName="jourPreference"]').first();
  89  |     if (await daySelect.isVisible({ timeout: 3000 }).catch(() => false)) {
  90  |       await daySelect.click({ force: true });
  91  |       await page.locator('mat-option').filter({ hasText: /vendredi|friday/i }).first().click();
  92  |     }
  93  | 
  94  |     const durationInput = page.locator('input[formControlName="duration"], input[formControlName="duree"]').first();
  95  |     if (await durationInput.isVisible({ timeout: 3000 }).catch(() => false)) {
  96  |       await durationInput.fill(String(c.duration));
  97  |     }
  98  | 
  99  |     // Next step
  100 |     const nextBtn2 = page.locator('button[matStepperNext], button').filter({ hasText: /suivant|next/i }).first();
  101 |     if (await nextBtn2.isVisible({ timeout: 3000 }).catch(() => false)) {
> 102 |       await nextBtn2.click();
      |                      ^ Error: locator.click: Test timeout of 60000ms exceeded.
  103 |       await page.waitForTimeout(500);
  104 |     }
  105 | 
  106 |     // Step 3: Payment terms
  107 |     const advanceInput = page.locator('input[formControlName="advancePayment"], input[formControlName="avance"]').first();
  108 |     if (await advanceInput.isVisible({ timeout: 5000 }).catch(() => false)) {
  109 |       await advanceInput.fill(String(c.advancePayment));
  110 |     }
  111 | 
  112 |     const penaltyInput = page.locator('input[formControlName="penaltyLate"], input[formControlName="penalite"]').first();
  113 |     if (await penaltyInput.isVisible({ timeout: 3000 }).catch(() => false)) {
  114 |       await penaltyInput.fill(String(c.penaltyLate));
  115 |     }
  116 | 
  117 |     // Next step
  118 |     const nextBtn3 = page.locator('button[matStepperNext], button').filter({ hasText: /suivant|next/i }).first();
  119 |     if (await nextBtn3.isVisible({ timeout: 3000 }).catch(() => false)) {
  120 |       await nextBtn3.click();
  121 |       await page.waitForTimeout(500);
  122 |     }
  123 | 
  124 |     // Step 4: Quality requirements
  125 |     const halalCheckbox = page.locator('mat-checkbox').filter({ hasText: /halal/i }).first();
  126 |     if (await halalCheckbox.isVisible({ timeout: 5000 }).catch(() => false)) {
  127 |       const isChecked = await halalCheckbox.locator('input[type="checkbox"]').isChecked();
  128 |       if (!isChecked) {
  129 |         await halalCheckbox.click();
  130 |       }
  131 |     }
  132 | 
  133 |     // Next step
  134 |     const nextBtn4 = page.locator('button[matStepperNext], button').filter({ hasText: /suivant|next/i }).first();
  135 |     if (await nextBtn4.isVisible({ timeout: 3000 }).catch(() => false)) {
  136 |       await nextBtn4.click();
  137 |       await page.waitForTimeout(500);
  138 |     }
  139 | 
  140 |     // Step 5: Confirm / Submit
  141 |     const submitBtn = page.locator('button[type="submit"], button').filter({ hasText: /cr[eé]er|finaliser|confirmer|submit|enregistrer/i }).first();
  142 |     if (await submitBtn.isVisible({ timeout: 5000 }).catch(() => false)) {
  143 |       await submitBtn.click();
  144 |       await page.waitForTimeout(1000);
  145 |     }
  146 | 
  147 |     await expect(page.locator('body')).toBeVisible();
  148 |   });
  149 | 
  150 |   // --------------------------------------------------
  151 |   // Contracts list
  152 |   // --------------------------------------------------
  153 |   test('Contract appears in active contracts list', async ({ page }) => {
  154 |     const eleveur = eleveurs[0];
  155 |     await loginAs(page, eleveur.email, eleveur.password);
  156 | 
  157 |     await navigateTo(page, '/contracts');
  158 |     await page.waitForLoadState('domcontentloaded');
  159 | 
  160 |     // Check for contract items in the list
  161 |     const contractItems = page.locator('mat-card, tr, .contract-item').filter({ hasText: /contrat|contract/i });
  162 |     // The list may or may not have items depending on API state
  163 |     await expect(page.locator('body')).toContainText(/contrat|contract/i, { timeout: 10000 });
  164 |   });
  165 | 
  166 |   // --------------------------------------------------
  167 |   // Client views contract
  168 |   // --------------------------------------------------
  169 |   test('Client sees contract in contracts list', async ({ page }) => {
  170 |     const client = clients[0];
  171 |     await loginAs(page, client.email, client.password);
  172 | 
  173 |     await navigateTo(page, '/contracts');
  174 |     await page.waitForLoadState('domcontentloaded');
  175 | 
  176 |     // Client should see the contracts page
  177 |     await expect(page.locator('body')).toContainText(/contrat|contract/i, { timeout: 10000 });
  178 |   });
  179 | 
  180 |   // --------------------------------------------------
  181 |   // Contract detail
  182 |   // --------------------------------------------------
  183 |   test('Eleveur can open contract detail page', async ({ page }) => {
  184 |     const eleveur = eleveurs[0];
  185 |     await loginAs(page, eleveur.email, eleveur.password);
  186 | 
  187 |     await navigateTo(page, '/contracts');
  188 |     await page.waitForLoadState('domcontentloaded');
  189 | 
  190 |     // Try to click on the first contract to open detail
  191 |     const contractLink = page.locator('a, mat-card, tr').filter({ hasText: /contrat|hebdo|mensuel/i }).first();
  192 |     if (await contractLink.isVisible({ timeout: 5000 }).catch(() => false)) {
  193 |       await contractLink.click();
  194 |       await page.waitForLoadState('domcontentloaded');
  195 | 
  196 |       // Contract detail should show frequency, quantity, etc.
  197 |       await expect(page.locator('body')).toBeVisible();
  198 |     }
  199 | 
  200 |     await expect(page.locator('body')).toBeVisible();
  201 |   });
  202 | });
```