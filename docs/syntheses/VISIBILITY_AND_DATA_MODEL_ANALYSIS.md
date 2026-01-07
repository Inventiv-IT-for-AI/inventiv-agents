# Analyse : Visibilité et Data Model Multi-Tenant

**Date** : 2025-01-XX  
**Objectif** : Clarifier la visibilité des modules/fonctions selon workspace, rôle, et souscription avant Phase 2

---

## 📊 Matrice de Visibilité Complète

### 1. Mode Personal (User sans Organisation)

#### 1.1 User Personal - Plan Free (`account_plan = 'free'`)

| Module | Voir | Créer | Modifier | Supprimer | Notes |
|--------|------|-------|----------|-----------|-------|
| **Chat** | ✅ | ✅ | ✅ | ✅ | Modèles publics gratuits uniquement |
| **Workbench** | ✅ | ✅ | ✅ | ✅ | Sessions/projets personnels |
| **API Keys** | ✅ | ✅ | ✅ | ✅ | Clés user-owned uniquement |
| **Models** | ✅ | ❌ | ❌ | ❌ | Modèles publics uniquement (gratuits) |
| **Instances** | ❌ | ❌ | ❌ | ❌ | **Non disponible** (org requis) |
| **Users** | ❌ | ❌ | ❌ | ❌ | **Non disponible** (org requis) |
| **Settings** | ❌ | ❌ | ❌ | ❌ | **Non disponible** (org requis) |
| **FinOps** | ✅ | ❌ | ❌ | ❌ | Coûts personnels uniquement (si wallet) |
| **Observability** | ❌ | ❌ | ❌ | ❌ | **Non disponible** (org requis) |
| **Monitoring** | ❌ | ❌ | ❌ | ❌ | **Non disponible** (org requis) |
| **Organizations** | ✅ | ✅ | ❌ | ❌ | Peut créer une org |

**Modèles accessibles** :
- ✅ Modèles publics avec `access_policy = 'free'`
- ❌ Modèles publics avec `access_policy = 'subscription_required'` (nécessite upgrade)
- ❌ Modèles `unlisted` ou `private` (nécessite entitlement)

**API Keys** :
- ✅ Créer clés user-owned avec scope limité aux modèles publics gratuits
- ❌ Créer clés org-owned (nécessite org)

**Wallet/Credits** :
- ✅ Provisionner wallet/solde tokens (pay-as-you-go)
- ✅ Consommer depuis wallet

---

#### 1.2 User Personal - Plan Subscriber (`account_plan = 'subscriber'`)

| Module | Voir | Créer | Modifier | Supprimer | Notes |
|--------|------|-------|----------|-----------|-------|
| **Chat** | ✅ | ✅ | ✅ | ✅ | Modèles publics (gratuits + abonnés) |
| **Workbench** | ✅ | ✅ | ✅ | ✅ | Sessions/projets personnels |
| **API Keys** | ✅ | ✅ | ✅ | ✅ | Clés user-owned uniquement |
| **Models** | ✅ | ❌ | ❌ | ❌ | Modèles publics (gratuits + abonnés) |
| **Instances** | ❌ | ❌ | ❌ | ❌ | **Non disponible** (org requis) |
| **Users** | ❌ | ❌ | ❌ | ❌ | **Non disponible** (org requis) |
| **Settings** | ❌ | ❌ | ❌ | ❌ | **Non disponible** (org requis) |
| **FinOps** | ✅ | ❌ | ❌ | ❌ | Coûts personnels uniquement |
| **Observability** | ❌ | ❌ | ❌ | ❌ | **Non disponible** (org requis) |
| **Monitoring** | ❌ | ❌ | ❌ | ❌ | **Non disponible** (org requis) |
| **Organizations** | ✅ | ✅ | ❌ | ❌ | Peut créer une org |

**Modèles accessibles** :
- ✅ Modèles publics avec `access_policy = 'free'`
- ✅ Modèles publics avec `access_policy = 'subscription_required'` (abonné)
- ❌ Modèles `unlisted` ou `private` (nécessite entitlement)
- ✅ Modèles avec `access_policy = 'request_required'` (peut demander accès)

**API Keys** :
- ✅ Créer clés user-owned avec scope étendu (modèles publics + abonnés)
- ❌ Créer clés org-owned (nécessite org)

**Wallet/Credits** :
- ✅ Provisionner wallet/solde tokens (pay-as-you-go)
- ✅ Consommer depuis wallet
- ✅ Consommer modèles `pay_per_token` (débit depuis wallet)

---

### 2. Mode Organisation (User membre d'une Organisation)

#### 2.1 User Organisation - Rôle User (`organization_role = 'user'`)

**Hypothèse** : L'org peut avoir un plan de souscription (`organization_subscription_plan`)

| Module | Voir | Créer | Modifier | Supprimer | Notes |
|--------|------|-------|----------|-----------|-------|
| **Chat** | ✅ | ✅ | ✅ | ✅ | Modèles org + publics selon plan org |
| **Workbench** | ✅ | ✅ | ✅ | ✅ | Sessions/projets org (peut partager) |
| **API Keys** | ✅ | ✅ | ✅ | ✅ | Clés user-owned uniquement |
| **Models** | ✅ | ❌ | ❌ | ❌ | Modèles org + publics selon plan org |
| **Instances** | ✅ | ❌ | ❌ | ❌ | Voir instances org uniquement |
| **Users** | ✅ | ❌ | ❌ | ❌ | Voir membres org uniquement |
| **Settings** | ❌ | ❌ | ❌ | ❌ | **Non disponible** (Admin/Owner) |
| **FinOps** | ❌ | ❌ | ❌ | ❌ | **Non disponible** (Manager/Owner) |
| **Observability** | ✅ | ❌ | ❌ | ❌ | Métriques instances org |
| **Monitoring** | ✅ | ❌ | ❌ | ❌ | Logs/events org |
| **Organizations** | ✅ | ✅ | ❌ | ❌ | Peut créer une autre org |

**Modèles accessibles** :
- ✅ Modèles org (`organization_id = org courante`)
- ✅ Modèles publics selon plan org :
  - Si org `subscription_plan = 'free'` → seulement modèles `free`
  - Si org `subscription_plan = 'subscriber'` → modèles `free` + `subscription_required`
- ✅ Modèles partagés avec org (`organization_model_shares` actifs)
- ❌ Modèles privés d'autres orgs (jamais visibles)

**API Keys** :
- ✅ Créer clés user-owned (scope limité selon rôle)
- ❌ Créer clés org-owned (Admin/Owner uniquement)

**Instances** :
- ✅ Voir instances org (`organization_id = org courante`)
- ✅ Voir métriques instances org
- ❌ Créer/modifier/terminer instances (Admin/Owner uniquement)

---

#### 2.2 User Organisation - Rôle Manager (`organization_role = 'manager'`)

| Module | Voir | Créer | Modifier | Supprimer | Notes |
|--------|------|-------|----------|-----------|-------|
| **Chat** | ✅ | ✅ | ✅ | ✅ | Modèles org + publics selon plan org |
| **Workbench** | ✅ | ✅ | ✅ | ✅ | Sessions/projets org |
| **API Keys** | ✅ | ✅ | ✅ | ✅ | Clés user-owned uniquement |
| **Models** | ✅ | ❌ | ❌ | ❌ | Modèles org + publics |
| **Instances** | ✅ | ❌ | ❌ | ❌ | Voir instances org |
| **Users** | ✅ | ✅ | ✅ | ⚠️ | Inviter users, changer rôle (Manager↔User) |
| **Settings** | ✅ | ❌ | ❌ | ❌ | Voir settings (lecture seule) |
| **FinOps** | ✅ | ❌ | ✅ | ❌ | **Gestion financière** : voir coûts, modifier prix, autoriser conso |
| **Observability** | ✅ | ❌ | ❌ | ❌ | Métriques instances org |
| **Monitoring** | ✅ | ❌ | ❌ | ❌ | Logs/events org |
| **Organizations** | ✅ | ✅ | ❌ | ❌ | Peut créer une autre org |

**Permissions spécifiques Manager** :
- ✅ **Activation économique** : Activer `eco_activated_by` sur ressources (instances, models, API keys)
- ✅ **Gestion prix** : Modifier prix d'achat instances, prix de vente offerings
- ✅ **Autorisation conso** : Autoriser instances en consommation, offerings en partage
- ✅ **Dashboards financiers** : Voir dépenses/recettes org
- ❌ **Activation technique** : Ne peut pas activer `tech_activated_by` (Admin/Owner)

**Modèles** :
- ✅ Voir modèles org + publics
- ✅ Voir dashboards financiers des modèles
- ❌ Créer/modifier modèles (Admin/Owner uniquement)
- ✅ Activer économiquement (`eco_activated_by`) modèles pour partage

**Instances** :
- ✅ Voir instances org
- ✅ Autoriser instances en consommation (activation économique)
- ❌ Créer/modifier/terminer instances (Admin/Owner uniquement)

---

#### 2.3 User Organisation - Rôle Admin (`organization_role = 'admin'`)

| Module | Voir | Créer | Modifier | Supprimer | Notes |
|--------|------|-------|----------|-----------|-------|
| **Chat** | ✅ | ✅ | ✅ | ✅ | Modèles org + publics selon plan org |
| **Workbench** | ✅ | ✅ | ✅ | ✅ | Sessions/projets org |
| **API Keys** | ✅ | ✅ | ✅ | ✅ | Clés user-owned + org-owned |
| **Models** | ✅ | ✅ | ✅ | ✅ | **Gestion complète** modèles org |
| **Instances** | ✅ | ✅ | ✅ | ✅ | **Gestion complète** instances org |
| **Users** | ✅ | ✅ | ✅ | ⚠️ | Inviter users, changer rôle (Admin↔User) |
| **Settings** | ✅ | ✅ | ✅ | ✅ | **Gestion infrastructure** : providers, regions, zones, types |
| **FinOps** | ✅ | ❌ | ❌ | ❌ | Voir dashboards (lecture seule) |
| **Observability** | ✅ | ❌ | ❌ | ❌ | Métriques instances org |
| **Monitoring** | ✅ | ❌ | ❌ | ❌ | Logs/events org |
| **Organizations** | ✅ | ✅ | ❌ | ❌ | Peut créer une autre org |

**Permissions spécifiques Admin** :
- ✅ **Activation technique** : Activer `tech_activated_by` sur ressources
- ✅ **Gestion infrastructure** : Providers, regions, zones, instance types, provider settings
- ✅ **Gestion instances** : Provision, install, reinstall, terminate
- ✅ **Gestion modèles** : Créer, modifier, publier offerings
- ✅ **Gestion API keys** : Créer/modifier clés org-owned
- ❌ **Activation économique** : Ne peut pas activer `eco_activated_by` (Manager/Owner)
- ❌ **Gestion prix** : Ne peut pas modifier prix (Manager/Owner)

**Modèles** :
- ✅ Créer/modifier/supprimer modèles org
- ✅ Publier offerings (`organization_models`)
- ✅ Activer techniquement (`tech_activated_by`) modèles
- ❌ Modifier prix offerings (Manager/Owner)

**Instances** :
- ✅ Créer/modifier/terminer instances org
- ✅ Activer techniquement (`tech_activated_by`) instances
- ❌ Autoriser économiquement instances (Manager/Owner)

---

#### 2.4 User Organisation - Rôle Owner (`organization_role = 'owner'`)

| Module | Voir | Créer | Modifier | Supprimer | Notes |
|--------|------|-------|----------|-----------|-------|
| **Chat** | ✅ | ✅ | ✅ | ✅ | Modèles org + publics selon plan org |
| **Workbench** | ✅ | ✅ | ✅ | ✅ | Sessions/projets org |
| **API Keys** | ✅ | ✅ | ✅ | ✅ | Clés user-owned + org-owned |
| **Models** | ✅ | ✅ | ✅ | ✅ | **Gestion complète** modèles org |
| **Instances** | ✅ | ✅ | ✅ | ✅ | **Gestion complète** instances org |
| **Users** | ✅ | ✅ | ✅ | ⚠️ | **Gestion complète** membres (sauf dernier owner) |
| **Settings** | ✅ | ✅ | ✅ | ✅ | **Gestion complète** infrastructure |
| **FinOps** | ✅ | ❌ | ✅ | ❌ | **Gestion financière** complète |
| **Observability** | ✅ | ❌ | ❌ | ❌ | Métriques instances org |
| **Monitoring** | ✅ | ❌ | ❌ | ❌ | Logs/events org |
| **Organizations** | ✅ | ✅ | ✅ | ⚠️ | Peut supprimer org (si pas dernière) |

**Permissions spécifiques Owner** :
- ✅ **Tout faire** : Activation technique + économique
- ✅ **Gestion membres** : Attribuer tous les rôles (Owner, Admin, Manager, User)
- ✅ **Gestion organisation** : Modifier nom/slug, supprimer org
- ✅ **Dernier owner** : Ne peut pas être retiré/downgradé (invariant)

**Modèles** :
- ✅ Tout ce que Admin peut faire
- ✅ Modifier prix offerings
- ✅ Activer économiquement (`eco_activated_by`)

**Instances** :
- ✅ Tout ce que Admin peut faire
- ✅ Autoriser économiquement instances

---

### 3. Impact du Plan de Souscription de l'Organisation

#### 3.1 Organisation - Plan Free (`organization_subscription_plan = 'free'`)

**Impact sur membres** :
- ✅ Accès aux modèles publics `free` uniquement
- ❌ Pas d'accès aux modèles `subscription_required`
- ✅ Peut demander accès à modèles `request_required`
- ✅ Peut consommer modèles `pay_per_token` (si wallet org)

**Limitations** :
- ⚠️ Pas de modèles premium dans le catalogue
- ⚠️ Pas d'offering `subscription_required` visible

---

#### 3.2 Organisation - Plan Subscriber (`organization_subscription_plan = 'subscriber'`)

**Impact sur membres** :
- ✅ Accès aux modèles publics `free`
- ✅ Accès aux modèles publics `subscription_required`
- ✅ Peut demander accès à modèles `request_required`
- ✅ Peut consommer modèles `pay_per_token` (si wallet org)

**Avantages** :
- ✅ Catalogue étendu (modèles premium)
- ✅ Offreings `subscription_required` visibles

---

## 🔍 Objets de Domaine Nécessaires

### 1. User Account Plan

**Table** : `users` (à enrichir)

**Colonnes à ajouter** :
```sql
ALTER TABLE users ADD COLUMN account_plan text DEFAULT 'free' NOT NULL;
ALTER TABLE users ADD COLUMN account_plan_updated_at timestamptz;
ALTER TABLE users ADD COLUMN wallet_balance_eur numeric(10,2) DEFAULT 0 NOT NULL;
ALTER TABLE users ADD CONSTRAINT users_account_plan_check CHECK (account_plan IN ('free', 'subscriber'));
```

**Valeurs** :
- `free` : Compte gratuit (accès modèles publics gratuits uniquement)
- `subscriber` : Compte abonné (accès modèles publics gratuits + abonnés)

**Logique** :
- Par défaut : `free`
- Upgrade : `free` → `subscriber` (via paiement/subscription)
- Downgrade : `subscriber` → `free` (si abonnement expiré)

---

### 2. Organization Subscription Plan

**Table** : `organizations` (à enrichir)

**Colonnes à ajouter** :
```sql
ALTER TABLE organizations ADD COLUMN subscription_plan text DEFAULT 'free' NOT NULL;
ALTER TABLE organizations ADD COLUMN subscription_plan_updated_at timestamptz;
ALTER TABLE organizations ADD COLUMN wallet_balance_eur numeric(10,2) DEFAULT 0 NOT NULL;
ALTER TABLE organizations ADD CONSTRAINT organizations_subscription_plan_check CHECK (subscription_plan IN ('free', 'subscriber'));
```

**Valeurs** :
- `free` : Organisation gratuite (accès modèles publics gratuits uniquement)
- `subscriber` : Organisation abonnée (accès modèles publics gratuits + abonnés)

**Logique** :
- Par défaut : `free`
- Impact sur tous les membres : Le plan org détermine les modèles accessibles
- Upgrade : `free` → `subscriber` (via paiement/subscription)
- Downgrade : `subscriber` → `free` (si abonnement expiré)

---

### 3. Double Activation (Tech/Eco)

**Tables à enrichir** : `instances`, `models`, `api_keys`, `organization_models`, etc.

**Colonnes à ajouter** :
```sql
-- Exemple pour instances
ALTER TABLE instances ADD COLUMN tech_activated_by uuid REFERENCES users(id);
ALTER TABLE instances ADD COLUMN tech_activated_at timestamptz;
ALTER TABLE instances ADD COLUMN eco_activated_by uuid REFERENCES users(id);
ALTER TABLE instances ADD COLUMN eco_activated_at timestamptz;
ALTER TABLE instances ADD COLUMN is_operational boolean GENERATED ALWAYS AS (
  tech_activated_by IS NOT NULL AND eco_activated_by IS NOT NULL
) STORED;
```

**Logique** :
- `tech_activated_by` : User (Admin/Owner) qui a activé techniquement
- `eco_activated_by` : User (Manager/Owner) qui a activé économiquement
- `is_operational` : `true` si les deux activations sont présentes

**Règles** :
- Owner peut activer tech + eco (mais doit faire les 2 activations explicitement)
- Admin peut activer tech uniquement
- Manager peut activer eco uniquement
- User ne peut rien activer

**Note importante** : Même si Owner a les deux rôles (Admin + Manager), il doit faire la double activation explicitement. C'est une règle de gouvernance pour éviter les erreurs.

---

## 📋 Terminologie Proposée

### Workspace
- **Personal** : Mode utilisateur sans organisation (`current_organization_id = NULL`)
- **Organization** : Mode utilisateur avec organisation (`current_organization_id != NULL`)

### Account Plan (User)
- **Free** : Compte gratuit (`account_plan = 'free'`)
- **Subscriber** : Compte abonné (`account_plan = 'subscriber'`)

### Subscription Plan (Organization)
- **Free** : Organisation gratuite (`subscription_plan = 'free'`)
- **Subscriber** : Organisation abonnée (`subscription_plan = 'subscriber'`)

### Organization Role
- **Owner** : Propriétaire (`organization_role = 'owner'`)
- **Admin** : Administrateur technique (`organization_role = 'admin'`)
- **Manager** : Gestionnaire financier (`organization_role = 'manager'`)
- **User** : Utilisateur (`organization_role = 'user'`)

### Model Visibility
- **Public** : Visible à tous (`visibility = 'public'`)
- **Unlisted** : Non listé mais accessible si autorisé (`visibility = 'unlisted'`)
- **Private** : Visible uniquement aux membres org (`visibility = 'private'`)

### Model Access Policy
- **Free** : Usage gratuit (`access_policy = 'free'`)
- **Subscription Required** : Réservé aux abonnés (`access_policy = 'subscription_required'`)
- **Request Required** : Demande d'accès requise (`access_policy = 'request_required'`)
- **Pay Per Token** : Facturation au token (`access_policy = 'pay_per_token'`)
- **Trial** : Gratuit jusqu'à date/quota (`access_policy = 'trial'`)

---

## ⚠️ Points à Clarifier / Décider

### 1. Plan User vs Plan Organisation ✅ CLARIFIÉ

**Règle** : Le plan s'applique selon le **workspace (session) actif** :

- **Session Personal** (`current_organization_id = NULL`) → `users.account_plan` s'applique
- **Session Organisation A** (`current_organization_id = org_a_id`) → `organizations.subscription_plan` (org A) s'applique
- **Session Organisation B** (`current_organization_id = org_b_id`) → `organizations.subscription_plan` (org B) s'applique

**Comportement** :
- Si le user switch de workspace (Personal ↔ Org A ↔ Org B), le plan qui s'applique change immédiatement
- Chaque session a son propre contexte de plan
- Pas de "prime" d'un plan sur l'autre : c'est le workspace actif qui détermine le plan

**Exemple** :
- User avec `account_plan = 'subscriber'` en session Personal → voit modèles `subscription_required`
- Même user en session Org A avec `subscription_plan = 'free'` → voit seulement modèles `free`
- Même user en session Org B avec `subscription_plan = 'subscriber'` → voit modèles `subscription_required`

---

### 2. Wallet User vs Wallet Organisation ✅ CLARIFIÉ

**Règle** : Le wallet s'applique selon le **workspace (session) actif** :

- **Session Personal** → débit depuis `users.wallet_balance_eur`
- **Session Organisation A** → débit depuis `organizations.wallet_balance_eur` (org A)
- **Session Organisation B** → débit depuis `organizations.wallet_balance_eur` (org B)

**Comportement** :
- Si le user switch de workspace, le wallet utilisé change immédiatement
- Chaque session a son propre contexte de wallet
- Trois wallets distincts possibles : 1 user wallet + N org wallets (une par org)

**Exemple** :
- User avec `wallet_balance_eur = 100€` en session Personal → consomme depuis wallet personnel
- Même user en session Org A avec `wallet_balance_eur = 50€` → consomme depuis wallet org A
- Même user en session Org B avec `wallet_balance_eur = 200€` → consomme depuis wallet org B

---

### 3. API Keys User-owned vs Org-owned

**Question** : Un user peut-il créer des clés org-owned s'il est Admin/Owner ?

**Proposition** : **Oui** - Les clés org-owned sont créées au nom de l'org et débitent depuis `organizations.wallet_balance_eur`.

**Scopes** :
- Clés user-owned : Scope limité selon `account_plan` user
- Clés org-owned : Scope limité selon `subscription_plan` org + permissions org

---

### 4. Modèles Org vs Modèles Publics

**Question** : Un user en mode Organisation voit-il les modèles org ET les modèles publics ?

**Proposition** : **Oui** - Union des deux :
- Modèles org (`organization_id = org courante`)
- Modèles publics (`organization_id IS NULL`) selon `subscription_plan` org

**Filtrage** :
- Modèles publics `free` → toujours visibles
- Modèles publics `subscription_required` → visibles si org `subscription_plan = 'subscriber'`
- Modèles `unlisted` → visibles si entitlement/share actif
- Modèles `private` → jamais visibles (org-only)

---

### 5. Instances Legacy ✅ CLARIFIÉ

**Règle** : **Pas de legacy à gérer** - On part sur un modèle propre.

**Comportement** :
- Application vierge avec seulement les comptes seed (default admin user + default Organisation)
- Pas de migration de données legacy
- Migration du data model uniquement (ajout colonnes, contraintes)
- Toutes les nouvelles instances auront `organization_id` défini dès la création

---

## 🎯 Plan d'Action Recommandé

### Étape 1 : Enrichir Data Model

**Migrations SQL** :
1. Ajouter `account_plan` à `users`
2. Ajouter `subscription_plan` à `organizations`
3. Ajouter `wallet_balance_eur` à `users` et `organizations`
4. Ajouter colonnes `tech_activated_by`, `eco_activated_by` aux ressources (instances, models, etc.)

### Étape 2 : Clarifier Logique de Visibilité

**Backend** :
1. Créer fonctions `can_access_model(user, model, workspace)` selon plan + rôle
2. Créer fonctions `can_view_module(user, module, workspace)` selon plan + rôle
3. Créer middleware RBAC réutilisable

**Frontend** :
1. Créer hooks `useCanAccess(permission)` selon workspace + rôle + plan
2. Masquer/afficher modules selon permissions
3. Afficher badges plan (Free/Subscriber) dans UI

### Étape 3 : Implémenter Phase 2 (Scoping Instances)

**Une fois le data model clarifié** :
1. Migration SQL : `instances.organization_id`
2. API : Filtrer instances selon workspace
3. Frontend : Badges, filtres, visibilité selon rôle

---

## ✅ Décisions Validées

1. **Plan User vs Plan Org** : ✅ Le plan s'applique selon le workspace (session) actif
2. **Wallet séparé** : ✅ Wallet selon le workspace (session) actif
3. **Instances legacy** : ✅ Pas de legacy - modèle propre dès le départ
4. **Double activation** : ✅ Implémenter dès Phase 2, Owner doit faire les 2 activations explicitement
5. **Terminologie** : ✅ Validée - À documenter dans `docs/domain_design_and_data_model.md`

---

**Prochaine étape** : Mettre à jour `docs/domain_design_and_data_model.md` avec la vision cible complète, puis lancer Phase 2.

