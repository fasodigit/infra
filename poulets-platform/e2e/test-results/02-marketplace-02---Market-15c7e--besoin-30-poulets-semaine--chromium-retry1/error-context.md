# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: 02-marketplace.spec.ts >> 02 - Marketplace >> Client creates a besoin (30 poulets/semaine)
- Location: tests/02-marketplace.spec.ts:153:7

# Error details

```
Test timeout of 60000ms exceeded.
```

```
Error: locator.click: Test timeout of 60000ms exceeded.
Call log:
  - waiting for locator('button[type="submit"], button').filter({ hasText: /publier|soumettre|creer|submit/i }).first()
    - locator resolved to <button type="submit" color="primary" disabled="true" mat-raised-button="" _ngcontent-ng-c1077121854="" mat-ripple-loader-disabled="" mat-ripple-loader-uninitialized="" mat-ripple-loader-class-name="mat-mdc-button-ripple" class="mdc-button mat-mdc-button-base mdc-button--raised mat-mdc-raised-button mat-primary mat-mdc-button-disabled">…</button>
  - attempting click action
    2 × waiting for element to be visible, enabled and stable
      - element is not enabled
    - retrying click action
    - waiting 20ms
    2 × waiting for element to be visible, enabled and stable
      - element is not enabled
    - retrying click action
      - waiting 100ms
    109 × waiting for element to be visible, enabled and stable
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
    - main [ref=e59]:
      - generic [ref=e61]:
        - heading "marketplace.besoins.create.title" [level=1] [ref=e63]:
          - img [ref=e64]: add_circle_outline
          - text: marketplace.besoins.create.title
        - generic [ref=e67]:
          - heading "marketplace.besoins.create.sectionWhat" [level=3] [ref=e68]
          - separator [ref=e69]
          - generic [ref=e73] [cursor=pointer]:
            - generic [ref=e74]:
              - text: marketplace.besoins.create.races
              - generic [ref=e75]: "*"
            - combobox "marketplace.besoins.create.races" [ref=e77]:
              - img [ref=e83]
          - generic [ref=e86]:
            - generic [ref=e89]:
              - generic [ref=e90]:
                - text: marketplace.besoins.create.quantity
                - generic [ref=e91]: "*"
              - spinbutton "marketplace.besoins.create.quantity" [ref=e93]: "30"
            - generic [ref=e97]:
              - generic [ref=e98]:
                - text: marketplace.besoins.create.minimumWeight (kg)
                - generic [ref=e99]: "*"
              - spinbutton "marketplace.besoins.create.minimumWeight (kg)" [ref=e101]: "2"
            - generic [ref=e105]:
              - generic [ref=e106]:
                - text: marketplace.besoins.create.deliveryDate
                - generic [ref=e107]: "*"
              - textbox "marketplace.besoins.create.deliveryDate" [ref=e109]
              - button "Open calendar" [ref=e112] [cursor=pointer]:
                - img [ref=e113]
          - heading "marketplace.besoins.create.sectionBudget" [level=3] [ref=e118]
          - separator [ref=e119]
          - generic [ref=e120]:
            - generic [ref=e123]:
              - generic [ref=e124]:
                - text: marketplace.besoins.create.maxBudgetPerKg (FCFA)
                - generic [ref=e125]: "*"
              - spinbutton "marketplace.besoins.create.maxBudgetPerKg (FCFA)" [ref=e127]: "4000"
            - generic [ref=e131]:
              - generic [ref=e132]:
                - text: marketplace.besoins.create.location
                - generic [ref=e133]: "*"
              - textbox "marketplace.besoins.create.location" [active] [ref=e135]: Ouagadougou, Zone du Bois
          - heading "marketplace.besoins.create.sectionFrequency" [level=3] [ref=e137]
          - separator [ref=e138]
          - generic [ref=e142] [cursor=pointer]:
            - generic [ref=e143]:
              - text: marketplace.besoins.create.frequency
              - generic [ref=e144]: "*"
            - combobox "marketplace.besoins.create.frequency" [ref=e146]:
              - generic [ref=e147]:
                - generic [ref=e149]: Ponctuel (une seule fois)
                - img [ref=e152]
          - heading "marketplace.besoins.create.sectionRequirements" [level=3] [ref=e155]
          - separator [ref=e156]
          - generic [ref=e159]:
            - generic [ref=e160] [cursor=pointer]:
              - checkbox "marketplace.besoins.create.halalRequired" [ref=e162]
              - generic:
                - img
            - generic [ref=e163] [cursor=pointer]: marketplace.besoins.create.halalRequired
          - generic [ref=e166]:
            - generic [ref=e167] [cursor=pointer]:
              - checkbox "marketplace.besoins.create.vetRequired" [ref=e169]
              - generic:
                - img
            - generic [ref=e170] [cursor=pointer]: marketplace.besoins.create.vetRequired
          - generic [ref=e171]:
            - generic [ref=e173]:
              - generic [ref=e174]: marketplace.besoins.create.specialNotes
              - textbox "marketplace.besoins.create.specialNotes" [ref=e176]: Livraison chaque vendredi matin avant 8h
            - generic [ref=e179]: marketplace.besoins.create.specialNotesHint
          - generic [ref=e181]:
            - button "Annuler" [ref=e182]:
              - generic [ref=e183]: Annuler
            - button "marketplace.besoins.create.submit" [disabled]:
              - generic:
                - img: publish
                - text: marketplace.besoins.create.submit
    - contentinfo [ref=e186]: FASO DIGITALISATION - Poulets Platform v0.1.0
```

# Test source

```ts
  96  |     await page.locator('input[formControlName="currentWeight"]').fill(String(a.currentWeight));
  97  |     await page.locator('input[formControlName="estimatedWeight"]').fill(String(a.estimatedWeight));
  98  |     await page.locator('input[formControlName="targetDate"]').fill(a.targetDate);
  99  |     await page.locator('input[formControlName="pricePerKg"]').fill(String(a.pricePerKg));
  100 |     await page.locator('input[formControlName="pricePerUnit"]').fill(String(a.pricePerUnit));
  101 |     await page.locator('input[formControlName="location"]').fill(a.location);
  102 | 
  103 |     const today = new Date().toISOString().split('T')[0];
  104 |     await page.locator('input[formControlName="availabilityStart"]').fill(today);
  105 |     await page.locator('input[formControlName="availabilityEnd"]').fill(a.targetDate);
  106 |     await page.locator('textarea[formControlName="description"]').fill(a.description);
  107 |     await page.locator('input[formControlName="ficheSanitaireId"]').fill(a.ficheSanitaireId);
  108 | 
  109 |     if (a.halalCertified) {
  110 |       const checkbox = page.locator('mat-checkbox[formControlName="halalCertified"]');
  111 |       const isChecked = await checkbox.locator('input[type="checkbox"]').isChecked();
  112 |       if (!isChecked) {
  113 |         await checkbox.click();
  114 |       }
  115 |     }
  116 | 
  117 |     await page.locator('button[type="submit"]').click();
  118 |     await expect(page.locator('body')).toContainText(/succes|annonce/i, { timeout: 10000 });
  119 |   });
  120 | 
  121 |   // --------------------------------------------------
  122 |   // Annonces list
  123 |   // --------------------------------------------------
  124 |   test('Annonces list shows created annonces', async ({ page }) => {
  125 |     const eleveur = eleveurs[0];
  126 |     await loginAs(page, eleveur.email, eleveur.password);
  127 | 
  128 |     await navigateTo(page, '/marketplace/annonces');
  129 |     await page.waitForLoadState('networkidle');
  130 | 
  131 |     // The page should contain annonce cards or a list
  132 |     // Check the page loaded properly
  133 |     await expect(page.locator('body')).toContainText(/annonce|marketplace/i, { timeout: 10000 });
  134 |   });
  135 | 
  136 |   // --------------------------------------------------
  137 |   // Client: Browse marketplace
  138 |   // --------------------------------------------------
  139 |   test('Client browses marketplace and sees annonces', async ({ page }) => {
  140 |     const client = clients[0];
  141 |     await loginAs(page, client.email, client.password);
  142 | 
  143 |     await navigateTo(page, '/marketplace');
  144 |     await page.waitForLoadState('networkidle');
  145 | 
  146 |     // The marketplace page should be visible
  147 |     await expect(page.locator('body')).toContainText(/marketplace|annonce/i, { timeout: 10000 });
  148 |   });
  149 | 
  150 |   // --------------------------------------------------
  151 |   // Client: Create besoin
  152 |   // --------------------------------------------------
  153 |   test('Client creates a besoin (30 poulets/semaine)', async ({ page }) => {
  154 |     const client = clients[0];
  155 |     await loginAs(page, client.email, client.password);
  156 | 
  157 |     await navigateTo(page, '/marketplace/besoins/new');
  158 |     await page.waitForLoadState('domcontentloaded');
  159 | 
  160 |     const b = besoins[0];
  161 | 
  162 |     // Fill besoin form fields - these depend on the actual form structure
  163 |     // Look for quantity input
  164 |     const quantityInput = page.locator('input[formControlName="quantity"], input[formControlName="quantite"]').first();
  165 |     if (await quantityInput.isVisible({ timeout: 5000 }).catch(() => false)) {
  166 |       await quantityInput.fill(String(b.quantity));
  167 |     }
  168 | 
  169 |     // Look for minimum weight
  170 |     const weightInput = page.locator('input[formControlName="minimumWeight"], input[formControlName="minWeight"]').first();
  171 |     if (await weightInput.isVisible({ timeout: 3000 }).catch(() => false)) {
  172 |       await weightInput.fill(String(b.minWeight));
  173 |     }
  174 | 
  175 |     // Budget
  176 |     const budgetInput = page.locator('input[formControlName="maxBudgetPerKg"], input[formControlName="budget"]').first();
  177 |     if (await budgetInput.isVisible({ timeout: 3000 }).catch(() => false)) {
  178 |       await budgetInput.fill(String(b.maxBudgetPerKg));
  179 |     }
  180 | 
  181 |     // Notes
  182 |     const notesInput = page.locator('textarea[formControlName="specialNotes"], textarea[formControlName="notes"]').first();
  183 |     if (await notesInput.isVisible({ timeout: 3000 }).catch(() => false)) {
  184 |       await notesInput.fill(b.notes);
  185 |     }
  186 | 
  187 |     // Location
  188 |     const locationInput = page.locator('input[formControlName="location"], input[formControlName="localisation"]').first();
  189 |     if (await locationInput.isVisible({ timeout: 3000 }).catch(() => false)) {
  190 |       await locationInput.fill(b.location);
  191 |     }
  192 | 
  193 |     // Try to submit the form
  194 |     const submitBtn = page.locator('button[type="submit"], button').filter({ hasText: /publier|soumettre|creer|submit/i }).first();
  195 |     if (await submitBtn.isVisible({ timeout: 3000 }).catch(() => false)) {
> 196 |       await submitBtn.click();
      |                       ^ Error: locator.click: Test timeout of 60000ms exceeded.
  197 |     }
  198 | 
  199 |     // Verify we are still on a valid page
  200 |     await expect(page.locator('body')).toBeVisible();
  201 |   });
  202 | 
  203 |   // --------------------------------------------------
  204 |   // Matching
  205 |   // --------------------------------------------------
  206 |   test('Client navigates to matching page', async ({ page }) => {
  207 |     const client = clients[0];
  208 |     await loginAs(page, client.email, client.password);
  209 | 
  210 |     await navigateTo(page, '/marketplace/matching');
  211 |     await page.waitForLoadState('domcontentloaded');
  212 | 
  213 |     // The matching page should load
  214 |     await expect(page.locator('body')).toContainText(/match|correspondance|score/i, { timeout: 10000 });
  215 |   });
  216 | 
  217 |   // --------------------------------------------------
  218 |   // Filters
  219 |   // --------------------------------------------------
  220 |   test('Filter annonces by race', async ({ page }) => {
  221 |     const client = clients[0];
  222 |     await loginAs(page, client.email, client.password);
  223 | 
  224 |     await navigateTo(page, '/marketplace/annonces');
  225 |     await page.waitForLoadState('networkidle');
  226 | 
  227 |     // Look for race filter (mat-select or dropdown)
  228 |     const raceFilter = page.locator('mat-select').filter({ hasText: /race/i }).first();
  229 |     if (await raceFilter.isVisible({ timeout: 5000 }).catch(() => false)) {
  230 |       await raceFilter.click();
  231 |       // Select a specific race
  232 |       const option = page.locator('mat-option').first();
  233 |       if (await option.isVisible({ timeout: 3000 }).catch(() => false)) {
  234 |         await option.click();
  235 |       }
  236 |     }
  237 | 
  238 |     // Verify page is still loaded
  239 |     await expect(page.locator('body')).toBeVisible();
  240 |   });
  241 | 
  242 |   test('Filter annonces by weight range', async ({ page }) => {
  243 |     const client = clients[0];
  244 |     await loginAs(page, client.email, client.password);
  245 | 
  246 |     await navigateTo(page, '/marketplace/annonces');
  247 |     await page.waitForLoadState('networkidle');
  248 | 
  249 |     // Look for weight filter inputs
  250 |     const weightMinInput = page.locator('input').filter({ hasText: /poids.*min|weight.*min/i }).first();
  251 |     if (await weightMinInput.isVisible({ timeout: 3000 }).catch(() => false)) {
  252 |       await weightMinInput.fill('1.5');
  253 |     }
  254 | 
  255 |     const weightMaxInput = page.locator('input').filter({ hasText: /poids.*max|weight.*max/i }).first();
  256 |     if (await weightMaxInput.isVisible({ timeout: 3000 }).catch(() => false)) {
  257 |       await weightMaxInput.fill('3.0');
  258 |     }
  259 | 
  260 |     await expect(page.locator('body')).toBeVisible();
  261 |   });
  262 | 
  263 |   test('Filter annonces by price', async ({ page }) => {
  264 |     const client = clients[0];
  265 |     await loginAs(page, client.email, client.password);
  266 | 
  267 |     await navigateTo(page, '/marketplace/annonces');
  268 |     await page.waitForLoadState('networkidle');
  269 | 
  270 |     // Look for price filter
  271 |     const priceInput = page.locator('input').filter({ hasText: /prix|price/i }).first();
  272 |     if (await priceInput.isVisible({ timeout: 3000 }).catch(() => false)) {
  273 |       await priceInput.fill('4000');
  274 |     }
  275 | 
  276 |     await expect(page.locator('body')).toBeVisible();
  277 |   });
  278 | });
  279 | 
```