# TESTING — Poulets Platform Frontend

> Selectors stable across UI refactors. The Playwright E2E suite at
> `INFRA/tests-e2e/` reads attributes documented here. Drift = bug.

## Testid taxonomy

All testids are **kebab-case**, **scoped per page**, and live on
`data-testid` HTML attributes. Never derive an id from an i18n label
(it breaks when the active language switches).

| Pattern                          | Use for                                            | Example                              |
|----------------------------------|----------------------------------------------------|--------------------------------------|
| `<page>-page`                    | Outer page container (one per route)               | `cart-page`                          |
| `<page>-list`                    | Top-level list / grid container                    | `annonces-list`                      |
| `<page>-list-item-<id>`          | Each list row (id = stable backend uuid)           | `annonces-list-item-abc-123`         |
| `<page>-search-input`            | Top-of-list search field                           | `messaging-search-input`             |
| `<page>-filter-<field>`          | Filter input / dropdown / chip                     | `annonces-filter-race`               |
| `<page>-filter-clear`            | Filter reset button                                | `annonces-filter-clear`              |
| `<page>-form`                    | Form root element                                  | `profile-edit-form`                  |
| `<page>-form-<field>`            | Form input / select / textarea                     | `profile-edit-form-name`             |
| `<page>-form-error-<field>`      | Field-level error message                          | `profile-edit-form-error-phone`      |
| `<page>-form-submit`             | Submit button                                      | `checkout-form-submit`               |
| `<page>-action-<verb>`           | Standalone CTA (not part of a form)                | `cart-action-checkout`               |
| `<page>-status-<value>`          | Status badge (value = enum, lowercased)            | `orders-status-en_attente`           |
| `<page>-modal-<name>`            | Modal / drawer overlay                             | `calendar-modal-event-detail`        |
| `<page>-detail`                  | Detail-view container                              | `orders-detail`                      |
| `<page>-detail-field-<name>`     | Specific detail-view field                         | `cart-detail-field-total`            |
| `nav-*`                          | Shared header / sidebar                            | `nav-drawer-toggle`, `nav-action-logout` |
| `<page>-empty`                   | Empty-state placeholder                            | `notifications-empty`                |

### Rules

1. **Scope first**: prefix every id with the page name (or `nav-` for shared chrome).
2. **No translation strings**: never embed `'reputation.title'` or French/Mooré words.
3. **Stable ids in lists**: bind `[attr.data-testid]="'foo-list-item-' + item.id"` with the backend uuid, not the array index.
4. **Lowercase enum values**: status badges use `.toLowerCase()` of the enum.
5. **Don't change visual output**: testids are invisible attributes — adding one must not change CSS, layout, or component logic.

## Page registry

| Route                         | Component                                                                | Key testids                                                                                                          |
|-------------------------------|--------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------------------------|
| `/marketplace/annonces`       | `annonces-list.component.ts`                                             | `annonces-page`, `annonces-list`, `annonces-list-item-<id>`, `annonces-form-filter`, `annonces-filter-race`, `annonces-filter-location`, `annonces-filter-clear`, `annonces-form-submit`, `annonces-action-publish`, `annonces-empty` |
| `/marketplace/besoins`        | `besoins-list.component.ts`                                              | `besoins-page`, `besoins-list`, `besoins-list-item-<id>`, `besoins-form-filter`, `besoins-filter-race`, `besoins-filter-location`, `besoins-filter-clear`, `besoins-form-submit`, `besoins-action-publish`, `besoins-empty` |
| `/profile`                    | `profile-home.component.ts`                                              | `profile-page`, `profile-detail`, `profile-detail-field-name`, `profile-detail-field-email`, `profile-action-edit`   |
| `/profile/edit`               | `profile-edit.component.ts`                                              | `profile-edit-page`, `profile-edit-form`, `profile-edit-form-name`, `profile-edit-form-phone`, `profile-edit-form-address`, `profile-edit-form-description`, `profile-edit-form-photo`, `profile-edit-form-submit`, `profile-edit-form-error-name`, `profile-edit-form-error-phone`, `profile-edit-action-back`, `profile-edit-action-cancel`, `profile-edit-action-change-avatar` |
| `/calendar`                   | `calendar-view.component.ts` (+ `calendar-home.component.ts` empty stub) | `calendar-page`, `calendar-list`, `calendar-list-item-<yyyy-mm-dd>`, `calendar-month-label`, `calendar-action-prev-month`, `calendar-action-next-month`, `calendar-action-today`, `calendar-action-create-event`, `calendar-modal-event-detail` |
| `/messaging`                  | `conversations-list.component.ts` + `chat-window.component.ts`           | `messaging-page`, `messaging-search-input`, `messaging-list`, `messaging-list-item-<id>`, `messaging-empty`, `messaging-detail`, `messaging-thread`, `messaging-thread-message-<id>`, `messaging-form`, `messaging-form-message`, `messaging-form-submit`, `messaging-action-back`, `messaging-action-attach` |
| `/orders`                     | `orders-list.component.ts` + `order-detail.component.ts`                 | `orders-page`, `orders-list`, `orders-list-item-<id>`, `orders-filter-status`, `orders-status-<value>`, `orders-action-create`, `orders-empty`, `orders-detail`, `orders-detail-field-numero`, `orders-action-back`, `orders-action-track`, `orders-action-confirm`, `orders-action-cancel` |
| `/cart`                       | `cart.component.ts`                                                      | `cart-page`, `cart-list`, `cart-list-item-<id>`, `cart-summary`, `cart-detail-field-count`, `cart-detail-field-subtotal`, `cart-detail-field-shipping`, `cart-detail-field-total`, `cart-action-checkout`, `cart-action-clear`, `cart-action-remove-<id>`, `cart-empty` |
| `/checkout`                   | `checkout.component.ts`                                                  | `checkout-page`, `checkout-form`, `checkout-form-delivery`, `checkout-form-payment`, `checkout-form-name`, `checkout-form-phone`, `checkout-form-address`, `checkout-form-payment-method`, `checkout-payment-orange-money`, `checkout-payment-moov-money`, `checkout-payment-cash`, `checkout-form-submit`, `checkout-success`, `checkout-empty` |
| `/notifications`              | `notifications-inbox.component.ts`                                       | `notifications-page`, `notifications-list`, `notifications-list-item-<id>`, `notifications-filter-tabs`, `notifications-filter-all`, `notifications-filter-unread`, `notifications-action-mark-all-read`, `notifications-action-clear-all`, `notifications-action-mark-read-<id>`, `notifications-action-delete-<id>`, `notifications-detail-field-count`, `notifications-empty` |
| `/contracts`                  | `contracts-list.component.ts` + `contract-detail.component.ts`           | `contracts-page`, `contracts-list`, `contracts-list-item-<id>`, `contracts-filter-tabs`, `contracts-status-<value>`, `contracts-action-create`, `contracts-empty-active`, `contracts-detail`, `contracts-action-accept`, `contracts-action-reject`, `contracts-action-renew` |
| `/reputation`                 | `reputation-view.component.ts`                                           | `reputation-page`, `reputation-list`, `reputation-list-item-<id>`, `reputation-rating-widget`, `reputation-detail-field-rating`, `reputation-detail-field-total`, `reputation-badges`, `reputation-badge-<key>` |
| `/map`                        | `breeders-map.component.ts` (legacy: `map-view.component.ts` at `/map/legacy`) | `map-page`, `map-container`, `map-canvas`, `map-loading`, `map-detail-field-count`, `map-action-list-view` (legacy adds `map-list`, `map-list-item-<id>`, `map-filter-role`, `map-filter-race`) |
| Shared header / nav           | `layout.component.ts`                                                    | `nav-container`, `nav-header`, `nav-drawer`, `nav-drawer-toggle`, `nav-menu`, `nav-menu-item-<route>`, `nav-logo`, `nav-language-switcher`, `nav-action-notifications`, `nav-profile-menu-toggle`, `nav-profile-menu`, `nav-action-profile`, `nav-action-messaging`, `nav-action-logout` |

## Adding a testid

When you create a new component or refactor an existing one:

1. **Pick the page name** (kebab-case, derived from route — e.g. `/cart` → `cart`).
2. **Identify the user-actionable elements**: container, lists, forms, action buttons, modals, status indicators.
3. **Apply the taxonomy table above**. Use a stable backend id for list items, never an array index.
4. **Add the attribute on the host element**: `data-testid="cart-action-checkout"` (static) or `[attr.data-testid]="'cart-list-item-' + item.id"` (dynamic).
5. **Mirror the addition** in `INFRA/tests-e2e/tests/20-ui-testid-coverage/testid-presence.spec.ts` (one row in the `PAGES` array) and in the **Page registry** above.

Do **not** modify CSS classes, component structure, or visual output when adding testids. They are invisible hooks.

## Running E2E

```bash
cd INFRA/tests-e2e

# Just the testid suite (smoke check, ~20s):
bunx playwright test tests/20-ui-testid-coverage --project=chromium-headless --workers=2

# Full UI suite alongside the gateway / OWASP / authz checks:
bunx playwright test tests/15-gateway tests/17-owasp-top10 tests/16-authz-opa \
  tests/20-ui-testid-coverage --project=chromium-headless

# All E2E:
bunx playwright test --project=chromium-headless
```

The Angular dev server (`ng serve` on `:4801`) auto-reloads on HTML
changes (~2s HMR). The testid suite targets the Angular SPA directly via
`baseURL: http://localhost:4801` — it does **not** go through the
ARMAGEDDON gateway (which fronts the Next.js BFF API surface only).

For pages behind the auth guard, the test asserts on either the page
testid (auth bypass) or the `login-form` testid (redirect-to-login). Both
outcomes prove the testid is wired in the bundle.
