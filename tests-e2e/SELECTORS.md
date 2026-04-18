# SELECTORS — FASO Poulets Frontend (e2e)

Audit live du DOM Angular (2026-04-18) contre `http://localhost:4801`.
Le frontend utilise Angular 19 Material + ReactiveForms et `formControlName`
est l'attribut le plus stable — préférer `input[formcontrolname="..."]`
aux libellés traduits i18n.

## Architecture générale

- **Stack**: frontend Angular à `localhost:4801`, BFF à `localhost:4800`,
  Kratos public `4433`, Mailpit `8025`.
- **Auth guard**: lit `AuthService.currentUser()` (signal in-memory).
  L'initialisation passe par `checkSession()` qui appelle `/api/auth/session`
  sur le BFF. **BUG confirmé** : le BFF retourne systématiquement 401 sur
  `/api/auth/session`, ce qui casse la persistance de session après
  `page.goto()` / reload. Les tests qui nécessitent un état connecté ne
  peuvent donc pas traverser une navigation via URL ; ils doivent se dérouler
  dans le même `page` sans reload, ou s'appuyer sur `page.evaluate()` pour
  poser l'état directement.
- **Role mapping** : le register frontend expose uniquement trois valeurs
  de rôle : `eleveur`, `client`, `producteur_aliment`. Les rôles `pharmacie`,
  `vaccins`, `aliments` (fixture `actors25`) sont regroupés dans le frontend
  sous `producteur_aliment` — les tests signup de ces rôles l'utilisent.

## Routes frontend

| Route                        | Composant                          | Guard      |
|------------------------------|------------------------------------|------------|
| `/auth/login`                | LoginComponent                     | public     |
| `/auth/register`             | RegisterComponent                  | public     |
| `/auth/forgot-password`      | ForgotPasswordComponent            | public     |
| `/auth/mfa`                  | MfaChallengeComponent              | public     |
| `/dashboard/eleveur`         | EleveurDashboardComponent          | authGuard  |
| `/dashboard/client`          | ClientDashboardComponent           | authGuard  |
| `/dashboard/producteur`      | ProducteurDashboardComponent       | authGuard  |
| `/dashboard/admin`           | AdminDashboardComponent            | authGuard  |
| `/profile`                   | ProfileHomeComponent / -view       | authGuard  |
| `/profile/edit`              | ProfileEditComponent               | authGuard  |
| `/profile/mfa`               | MfaSettingsComponent               | authGuard  |
| `/profile/security`          | SecuritySessionsComponent          | authGuard  |
| `/marketplace`               | MarketplaceHomeComponent           | authGuard  |
| `/marketplace/annonces`      | AnnoncesListComponent              | authGuard  |
| `/marketplace/annonces/new`  | CreateAnnonceComponent             | authGuard  |
| `/marketplace/besoins`       | BesoinsListComponent               | authGuard  |
| `/marketplace/besoins/new`   | CreateBesoinComponent              | authGuard  |
| `/messaging`                 | ConversationsListComponent         | authGuard  |
| `/messaging/:id`             | ChatWindowComponent                | authGuard  |
| `/checkout`                  | Checkout routes                    | authGuard  |

**Note** : `/settings/security` n'existe pas ; le composant Security est
à `/profile/mfa`. Les URL historiques `/messages` n'existent pas non plus —
utiliser `/messaging`.

## SignupPage (`/auth/register`)

Formulaire **Angular Material stepper** à 4 étapes, toutes les étapes sont
rendues en DOM dès le départ mais Playwright filtre proprement via `:visible`.

### Sélecteurs (tous via `formcontrolname` — stable, indépendant de l'i18n)

| Champ              | Sélecteur                                              | Étape |
|--------------------|--------------------------------------------------------|-------|
| Nom complet        | `input[formcontrolname="nom"]`                         | 1     |
| Email              | `input[formcontrolname="email"]`                       | 1     |
| Téléphone          | `input[formcontrolname="phone"]` (facultatif)          | 1     |
| Mot de passe       | `input[formcontrolname="password"]`                    | 1     |
| Confirmation       | `input[formcontrolname="confirmPassword"]`             | 1     |
| Rôle (radio)       | `input[formcontrolname="role"][value="<role>"]`        | 2     |
| Localisation       | `input[formcontrolname="localisation"]`                | 3     |
| Capacité éleveur   | `input[formcontrolname="capacite"]`                    | 3     |
| Type client        | `select[formcontrolname="clientType"]`                 | 3     |
| Zone producteur    | `input[formcontrolname="zoneDistribution"]`            | 3     |
| Groupement         | `input[formcontrolname="groupementNom"]`               | 4     |

### Boutons

| Action                    | Sélecteur                                                       |
|---------------------------|-----------------------------------------------------------------|
| Continuer (étape courante)| `button:visible:has-text("Continuer")` (Playwright `:visible` filtre l'étape active) |
| Précédent                 | `button:has-text("Précédent"):visible`                          |
| Soumettre                 | `button:has-text("Créer mon compte")`                           |

### Valeurs radio rôle

- `eleveur` — carte **Éleveur**
- `client` — carte **Client**
- `producteur_aliment` — carte **Producteur** (absorbe `pharmacie`, `vaccins`, `aliments`)

### Assertions succès

Après submit réussi, `AuthService.register()` déclenche `navigateByRole()` :
- `eleveur` → `/dashboard/eleveur`
- `client` → `/dashboard/client`
- `producteur_aliment` → `/dashboard/producteur`

```typescript
await expect(page).toHaveURL(/\/dashboard(\/(eleveur|client|producteur|admin))?/);
```

Aucune étape OTP n'est actuellement enforcée dans le frontend — l'email Kratos
est bien envoyé (Mailpit reçoit « Please verify your email address » avec un
code à 6 chiffres), mais le frontend saute directement au dashboard.

## LoginPage (`/auth/login`)

| Champ          | Sélecteur                            |
|----------------|--------------------------------------|
| Email          | `input[formcontrolname="email"]`     |
| Mot de passe   | `input[formcontrolname="password"]`  |
| Submit         | `button[type="submit"]` (texte « Se connecter ») |
| Erreur         | `.error[role="alert"]`               |

## ProfileEditPage (`/profile/edit`)

**Guard** : authGuard redirige `/auth/login?returnUrl=/profile/edit`
tant que `currentUser()` est nul (bug BFF).

| Champ         | Sélecteur                                 |
|---------------|-------------------------------------------|
| Nom           | `input[formcontrolname="nom"]` (matInput) |
| Téléphone     | `input[formcontrolname="phone"]`          |
| Localisation  | `input[formcontrolname="localisation"]`   |
| Description   | `textarea[formcontrolname="description"]` |
| Save          | `button[type="submit"]` (texte « Save »)  |
| Avatar upload | `input[type="file"][hidden]`              |

**Non exposé** : SIRET, AMM, licence — ces champs attendus par l'ancien
`ProfilePage.ts` n'existent pas dans le composant actuel. Bug frontend
à remonter : le template `profile-edit.component` n'inclut que
`nom / phone / localisation / description`.

## SecurityPage / MfaSettingsPage (`/profile/mfa`)

**Guard** : authGuard.

Boutons principaux (targetés par texte FR) :

| Action              | Sélecteur                                         |
|---------------------|---------------------------------------------------|
| Ajouter PassKey     | `button:has-text("Ajouter une clé de sécurité")`  |
| Configurer TOTP     | `button:has-text("Configurer")` (card TOTP)       |
| Désactiver TOTP     | `button:has-text("Désactiver")`                   |
| Générer 10 codes    | `button:has-text("Générer 10 codes")`             |
| Régénérer codes     | `button:has-text("Régénérer mes codes")`          |
| Modifier email      | `button:has-text("Modifier l'email")`             |
| Supprimer passkey   | `.devices button[aria-label="Supprimer"]`         |

### Dialog TOTP (ouvert par « Configurer »)

| Élément            | Sélecteur                                       |
|--------------------|-------------------------------------------------|
| QR code            | `qrcode canvas` (angularx-qrcode)               |
| Secret affiché     | `.totp-dialog code` (contient secret base32)    |
| Input code 6-chars | `.totp-dialog input[type="text"]`               |
| Bouton Activer     | `button:has-text("Activer TOTP")`               |
| Bouton Annuler     | `button:has-text("Annuler")`                    |

### Dialog Backup codes

| Élément              | Sélecteur                                        |
|----------------------|--------------------------------------------------|
| Liste codes          | `.codes-dialog code`                             |
| Bouton « Conservé »  | `button:has-text("J'ai conservé mes codes")`     |
| Télécharger          | `button:has-text("Télécharger .txt")`            |

## MarketplacePage — CreateAnnonce (`/marketplace/annonces/new`)

**Guard** : authGuard.

| Champ              | Sélecteur                                                     |
|--------------------|---------------------------------------------------------------|
| Race               | `mat-select[formcontrolname="race"]`                          |
| Quantité           | `input[formcontrolname="quantity"]`                           |
| Poids actuel       | `input[formcontrolname="currentWeight"]`                      |
| Poids estimé       | `input[formcontrolname="estimatedWeight"]`                    |
| Date cible         | `input[formcontrolname="targetDate"]`                         |
| Prix /kg           | `input[formcontrolname="pricePerKg"]`                         |
| Prix /unité        | `input[formcontrolname="pricePerUnit"]`                       |
| Localisation       | `input[formcontrolname="location"]`                           |
| Dispo début        | `input[formcontrolname="availabilityStart"]`                  |
| Dispo fin          | `input[formcontrolname="availabilityEnd"]`                    |
| Description        | `textarea[formcontrolname="description"]`                     |
| Fiche sanitaire    | `input[formcontrolname="ficheSanitaireId"]`                   |
| Halal (checkbox)   | `mat-checkbox[formcontrolname="halalCertified"]`              |
| Publier comme      | `mat-select[formcontrolname="isGroupement"]`                  |
| Submit             | `button[type="submit"]` (texte « publish » / traduit)         |

### Sélection mat-select

```typescript
await page.locator('mat-select[formcontrolname="race"]').click();
await page.locator('mat-option').filter({ hasText: 'Bicyclette' }).click();
```

## MarketplacePage — CreateBesoin (`/marketplace/besoins/new`)

| Champ              | Sélecteur                                                     |
|--------------------|---------------------------------------------------------------|
| Races (multi)      | `mat-select[formcontrolname="races"]` (multiple)              |
| Quantité           | `input[formcontrolname="quantity"]`                           |
| Poids min          | `input[formcontrolname="minimumWeight"]`                      |
| Date livraison     | `input[formcontrolname="deliveryDate"]`                       |
| Budget /kg         | `input[formcontrolname="maxBudgetPerKg"]`                     |
| Localisation       | `input[formcontrolname="location"]`                           |
| Fréquence          | `mat-select[formcontrolname="frequency"]`                     |
| Submit             | `button[type="submit"]` (« publier besoin »)                  |

## MessagingPage (`/messaging`)

**Conversations list** (`/messaging`) :

Pas de `data-testid` exposé. La liste est rendue dans `ConversationsListComponent`
(à re-inspecter pour sélecteurs fins). Liens vers une conversation :
`a[routerlink*="/messaging/"]`.

**Chat window** (`/messaging/:conversationId`) :

| Élément         | Sélecteur                                          |
|-----------------|----------------------------------------------------|
| Nom interlocuteur| `.peer strong`                                    |
| Input message   | `input[name="draft"]` (type text, ngModel)         |
| Bouton envoyer  | `button[type="submit"]` dans `form.composer`       |
| Liste messages  | `.thread .msg .bubble`                             |
| Message envoyé  | `.msg.mine .bubble`                                |

## DashboardPage (`/dashboard/*`)

| Élément         | Sélecteur                                          |
|-----------------|----------------------------------------------------|
| Titre H1        | `h1` (textes variés selon rôle)                    |
| KPI cards       | `.kpi-card`                                        |
| Navigation      | `nav[role="navigation"]` (sidebar layout)          |
| Bouton logout   | `button:has-text("Déconnexion")` (dans user menu)  |

## Bugs frontend détectés pendant l'audit

1. **BFF `/api/auth/session` renvoie 401 sur cookie Kratos valide** —
   bloque la persistance de session entre reloads.
   Impact : tests authentifiés (marketplace, profile, security, messaging)
   ne peuvent pas s'exécuter sur un flow reload. Mitigation : dérouler
   le test dans un seul `page` sans reload, ou mocker côté Playwright.
   **Action** : ouvrir ticket BFF auth-ms / equipe.

2. **Rôles métiers regroupés** — le register UI mappe `pharmacie`, `vaccins`,
   `aliments` sur la seule carte « Producteur » (valeur `producteur_aliment`).
   Les tests signup de ces rôles utilisent ce bucket partagé. Ajouter des
   cartes dédiées pour distinguer ces rôles.

3. **ProfileEdit manque SIRET / AMM / licence upload** — le composant
   n'expose que `nom / phone / localisation / description`. Les tests
   acceptance SIRET/AMM marquent `test.fixme()` avec TODO.

4. **OTP verification UI absente** — Kratos envoie bien l'email avec code
   mais le frontend n'a pas de page de saisie. Les tests signup vérifient
   juste la navigation vers dashboard + la présence de l'email dans Mailpit.
