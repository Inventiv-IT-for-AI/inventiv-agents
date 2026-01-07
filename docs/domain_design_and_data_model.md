# Domain Design & Data Structures (DDD)

**Date de mise à jour** : 2025-01-XX  
**Vision** : Multi-tenant avec Users first-class + Organisations + RBAC + Double Activation

---

## 1. Ubiquitous Language (Langage Commun)

### Infrastructure & Compute
*   **Provider**: Un fournisseur d'infrastructure (ex: Scaleway, AWS, Mock).
*   **Instance (Node)**: Une machine virtuelle ou bare-metal fournie par un Provider. Elle possède une IP et des ressources GPU.
*   **Worker**: Le processus (Conteneur) qui s'exécute sur une Instance pour servir des modèles.
*   **Model**: Un modèle LLM spécifique (ex: `llama-3-70b-instruct`) avec des pré-requis techniques.
*   **Deployment**: L'association d'un Modèle sur une Instance.

### Multi-Tenant & Workspace
*   **Workspace**: Le contexte actif d'un utilisateur (Personal ou Organisation).
  *   **Personal**: Mode utilisateur sans organisation (`current_organization_id = NULL`)
  *   **Organization**: Mode utilisateur avec organisation (`current_organization_id != NULL`)
*   **Session**: Une session utilisateur avec un workspace spécifique (peut avoir plusieurs sessions simultanées avec des workspaces différents).

### Account & Subscription Plans
*   **Account Plan (User)**: Plan de souscription d'un utilisateur (`free` | `subscriber`).
  *   **Free**: Compte gratuit (`account_plan = 'free'`)
  *   **Subscriber**: Compte abonné (`account_plan = 'subscriber'`)
*   **Subscription Plan (Organization)**: Plan de souscription d'une organisation (`free` | `subscriber`).
  *   **Free**: Organisation gratuite (`subscription_plan = 'free'`)
  *   **Subscriber**: Organisation abonnée (`subscription_plan = 'subscriber'`)

**Règle importante** : Le plan s'applique selon le **workspace (session) actif** :
- Session Personal → `users.account_plan` s'applique
- Session Organisation A → `organizations.subscription_plan` (org A) s'applique
- Session Organisation B → `organizations.subscription_plan` (org B) s'applique
- Si switch de workspace, le plan change immédiatement

### Wallet & Billing
*   **Wallet User**: Solde tokens personnel (`users.wallet_balance_eur`).
*   **Wallet Organisation**: Solde tokens organisation (`organizations.wallet_balance_eur`).

**Règle importante** : Le wallet utilisé dépend du **workspace (session) actif** :
- Session Personal → débit depuis `users.wallet_balance_eur`
- Session Organisation A → débit depuis `organizations.wallet_balance_eur` (org A)
- Session Organisation B → débit depuis `organizations.wallet_balance_eur` (org B)

### Organization Roles (RBAC)
*   **Owner**: Propriétaire (`organization_role = 'owner'`) - Peut tout faire, doit faire double activation explicitement.
*   **Admin**: Administrateur technique (`organization_role = 'admin'`) - Gère infrastructure, instances, models, peut activer tech uniquement.
*   **Manager**: Gestionnaire financier (`organization_role = 'manager'`) - Gère finances, prix, autorisations, peut activer eco uniquement.
*   **User**: Utilisateur (`organization_role = 'user'`) - Utilise les ressources, pas de permissions d'administration.

### Model Visibility & Access
*   **Visibility**: Qui peut *voir* l'offering (`public` | `unlisted` | `private`).
  *   **Public**: Visible à tous (`visibility = 'public'`)
  *   **Unlisted**: Non listé mais accessible si autorisé (`visibility = 'unlisted'`)
  *   **Private**: Visible uniquement aux membres org (`visibility = 'private'`)
*   **Access Policy**: Dans quelles conditions on peut *utiliser* l'offering (`free` | `subscription_required` | `request_required` | `pay_per_token` | `trial`).
  *   **Free**: Usage gratuit (`access_policy = 'free'`)
  *   **Subscription Required**: Réservé aux abonnés (`access_policy = 'subscription_required'`)
  *   **Request Required**: Demande d'accès requise (`access_policy = 'request_required'`)
  *   **Pay Per Token**: Facturation au token (`access_policy = 'pay_per_token'`)
  *   **Trial**: Gratuit jusqu'à date/quota (`access_policy = 'trial'`)

### Double Activation
*   **Tech Activation**: Activation technique (`tech_activated_by`, `tech_activated_at`) - Admin/Owner uniquement.
*   **Eco Activation**: Activation économique (`eco_activated_by`, `eco_activated_at`) - Manager/Owner uniquement.
*   **Operational**: Ressource opérationnelle (`is_operational = true`) - Requiert les deux activations.

**Règle importante** : Même si Owner a les deux rôles (Admin + Manager), il doit faire la double activation explicitement. C'est une règle de gouvernance pour éviter les erreurs.

## 2. Domain Entities (Rust Structs)

Ces structures seront définies dans `inventiv-common`.

### A. Core Entities

#### `LlmModel` (Aggregate Root)
Définit un modèle disponible dans le catalogue.
```rust
pub struct LlmModel {
    pub id: Uuid,
    pub name: String,             // e.g., "Llama 3 70B"
    pub model_id: String,         // e.g., "meta-llama/Meta-Llama-3-70B-Instruct" (HuggingFace ID)
    pub updated_at: DateTime<Utc>,
    // Hardware Requirements (Value Object)
    pub required_vram_gb: i32,
    pub context_length: i32,
}
```

#### `Instance` (Entity)
Représente une ressource compute provisionnée.
```rust
pub struct Instance {
    pub id: Uuid,
    pub provider_id: String,      // ID côté Cloud Provider (ex: i-123456)
    pub provider_name: String,    // "scaleway", "aws"
    pub ip_address: Option<String>,
    pub status: InstanceStatus,
    pub created_at: DateTime<Utc>,
    // Hardware Specs (Value Object)
    pub gpu_profile: GPUProfile,  // e.g. { name: "H100", vram: 80 }
}
```

#### `InstanceStatus` (Enum/State Machine)
Cycle de vie rigoureux avec transitions explicites.

**États principaux** :
*   `Provisioning`: Demandé au provider, en attente.
*   `Booting`: Instance up, mais Worker pas encore prêt.
*   `Ready`: Worker prêt à recevoir du trafic (Healthcheck OK).
*   `Draining`: En cours d'arrêt, ne prend plus de nouvelles requêtes.
*   `Terminating`: En cours de suppression chez le provider.
*   `Terminated`: Détruite chez le provider.
*   `Archived`: Archivée (supprimée de la vue active).

**États d'erreur** :
*   `ProvisioningFailed`: Échec lors de la création de l'instance chez le provider.
*   `StartupFailed`: Échec lors du démarrage ou de la configuration du worker.
*   `Failed`: État générique d'échec.

**Transitions** : Gérées par des fonctions explicites dans `inventiv-orchestrator/src/state_machine.rs`.

> **Voir** : [docs/STATE_MACHINE_AND_PROGRESS.md](STATE_MACHINE_AND_PROGRESS.md) pour les détails complets sur les transitions, l'historique, et le progress tracking.

## 3. Storage Strategy

Nous séparons le "Cold Storage" (Configuration/Historique) du "Hot Storage" (Routing Temps Réel).

### A. PostgreSQL (System of Record - Orchestrator)
Gestion de la vérité terrain et de l'historique.

```sql
-- users (multi-tenant: users first-class)
CREATE TABLE users (
    id UUID PRIMARY KEY,
    email VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    username TEXT NOT NULL,
    role VARCHAR(50) DEFAULT 'admin',  -- User global role (admin|user)
    account_plan TEXT DEFAULT 'free' NOT NULL,  -- Account plan (free|subscriber)
    account_plan_updated_at TIMESTAMPTZ,
    wallet_balance_eur NUMERIC(10,2) DEFAULT 0 NOT NULL,  -- Wallet personnel
    first_name TEXT,
    last_name TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT users_account_plan_check CHECK (account_plan IN ('free', 'subscriber'))
);

-- organizations (multi-tenant workspaces)
CREATE TABLE organizations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    subscription_plan TEXT DEFAULT 'free' NOT NULL,  -- Subscription plan (free|subscriber)
    subscription_plan_updated_at TIMESTAMPTZ,
    wallet_balance_eur NUMERIC(10,2) DEFAULT 0 NOT NULL,  -- Wallet organisation
    sidebar_color TEXT,  -- Couleur sidebar configurable (UX anti-erreur)
    created_by_user_id UUID REFERENCES users(id),
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT organizations_subscription_plan_check CHECK (subscription_plan IN ('free', 'subscriber'))
);

-- organization_memberships (RBAC)
CREATE TABLE organization_memberships (
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role TEXT NOT NULL DEFAULT 'user',
    created_at TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (organization_id, user_id),
    CONSTRAINT organization_memberships_role_check CHECK (role IN ('owner', 'admin', 'manager', 'user'))
);

-- user_sessions (multi-sessions avec workspace)
CREATE TABLE user_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    current_organization_id UUID REFERENCES organizations(id) ON DELETE SET NULL,
    organization_role TEXT CHECK (organization_role IN ('owner', 'admin', 'manager', 'user')),
    session_token_hash TEXT NOT NULL,
    ip_address INET,
    user_agent TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ
);

-- providers (catalog)
CREATE TABLE providers (
    id UUID PRIMARY KEY,
    name VARCHAR(50) UNIQUE NOT NULL, -- "scaleway", "aws"
    description TEXT
);

-- regions
CREATE TABLE regions (
    id UUID PRIMARY KEY,
    provider_id UUID REFERENCES providers(id),
    name VARCHAR(50) NOT NULL, -- "fr-par", "us-east-1"
    UNIQUE(provider_id, name)
);

-- zones
CREATE TABLE zones (
    id UUID PRIMARY KEY,
    region_id UUID REFERENCES regions(id),
    name VARCHAR(50) NOT NULL, -- "fr-par-1"
    UNIQUE(region_id, name)
);

-- instance_types (catalog capabilities)
CREATE TABLE instance_types (
    id UUID PRIMARY KEY,
    provider_id UUID REFERENCES providers(id),
    name VARCHAR(50) NOT NULL, -- "H100-1-80G"
    gpu_count INT NOT NULL,
    vram_per_gpu_gb INT NOT NULL,
    UNIQUE(provider_id, name)
);

-- instance_availability (linking types to zones)
CREATE TABLE instance_availability (
    instance_type_id UUID REFERENCES instance_types(id),
    zone_id UUID REFERENCES zones(id),
    PRIMARY KEY(instance_type_id, zone_id)
);

-- ssh_keys (per provider)
CREATE TABLE ssh_keys (
    id UUID PRIMARY KEY,
    name VARCHAR(50) NOT NULL,
    public_key TEXT NOT NULL,
    provider_id UUID REFERENCES providers(id),
    provider_key_id VARCHAR(255), -- ID remote chez le provider
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- models
CREATE TABLE models (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    model_id VARCHAR(255) UNIQUE NOT NULL, -- "llama-3-8b"
    required_vram_gb INT NOT NULL,
    is_active BOOLEAN DEFAULT true
);

-- instances (org-scopées avec double activation)
CREATE TABLE instances (
    id UUID PRIMARY KEY,
    provider_id UUID REFERENCES providers(id),
    zone_id UUID REFERENCES zones(id),
    instance_type_id UUID REFERENCES instance_types(id),
    organization_id UUID REFERENCES organizations(id) ON DELETE SET NULL,  -- Org-scopé
    
    provider_instance_id VARCHAR(255),  -- ID distant
    ip_address INET,
    
    api_key VARCHAR(255), -- Key to call the worker securely
    
    status VARCHAR(50) NOT NULL,
    
    -- Double activation (tech + eco)
    tech_activated_by UUID REFERENCES users(id),
    tech_activated_at TIMESTAMPTZ,
    eco_activated_by UUID REFERENCES users(id),
    eco_activated_at TIMESTAMPTZ,
    is_operational BOOLEAN GENERATED ALWAYS AS (
        tech_activated_by IS NOT NULL AND eco_activated_by IS NOT NULL
    ) STORED,
    
    created_at TIMESTAMPTZ DEFAULT NOW(),
    terminated_at TIMESTAMPTZ,
    gpu_profile JSONB NOT NULL -- Snapshot des specs
);

-- models (org-scopés avec double activation)
CREATE TABLE models (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    model_id VARCHAR(255) UNIQUE NOT NULL, -- "llama-3-8b"
    required_vram_gb INT NOT NULL,
    organization_id UUID REFERENCES organizations(id) ON DELETE SET NULL,  -- Org-scopé (NULL = public)
    
    -- Double activation (tech + eco)
    tech_activated_by UUID REFERENCES users(id),
    tech_activated_at TIMESTAMPTZ,
    eco_activated_by UUID REFERENCES users(id),
    eco_activated_at TIMESTAMPTZ,
    is_operational BOOLEAN GENERATED ALWAYS AS (
        tech_activated_by IS NOT NULL AND eco_activated_by IS NOT NULL
    ) STORED,
    
    is_active BOOLEAN DEFAULT true
);

-- organization_models (offerings publiés par orgs)
CREATE TABLE organization_models (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    model_id UUID NOT NULL REFERENCES models(id) ON DELETE CASCADE,
    code TEXT NOT NULL,  -- Identifiant org-scopé (ex: "sales-bot")
    visibility TEXT NOT NULL DEFAULT 'private',  -- public|unlisted|private
    access_policy TEXT NOT NULL DEFAULT 'free',  -- free|subscription_required|request_required|pay_per_token|trial
    is_active BOOLEAN DEFAULT true NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(organization_id, code),
    CONSTRAINT organization_models_visibility_check CHECK (visibility IN ('public', 'unlisted', 'private')),
    CONSTRAINT organization_models_access_policy_check CHECK (access_policy IN ('free', 'subscription_required', 'request_required', 'pay_per_token', 'trial'))
);

-- organization_model_shares (contrats provider→consumer)
CREATE TABLE organization_model_shares (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider_organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    consumer_organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    organization_model_id UUID NOT NULL REFERENCES organization_models(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'active',  -- active|paused|revoked
    pricing JSONB NOT NULL,  -- { "version": 1, "type": "per_1k_tokens", "eur_per_1k": 0.2 }
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT organization_model_shares_distinct_orgs CHECK (provider_organization_id <> consumer_organization_id),
    CONSTRAINT organization_model_shares_status_check CHECK (status IN ('active', 'paused', 'revoked'))
);

-- api_keys (user-owned ou org-owned)
CREATE TABLE api_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    organization_id UUID REFERENCES organizations(id) ON DELETE SET NULL,  -- NULL = user-owned, NOT NULL = org-owned
    name TEXT NOT NULL,
    key_prefix TEXT NOT NULL,
    key_hash TEXT NOT NULL,
    scopes JSONB,  -- Restrictions (allowlist offerings, max spend, expiry)
    created_at TIMESTAMPTZ DEFAULT NOW(),
    last_used_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ
);
```

### B. Redis (Real-time Discovery - Router & Orchestrator)
Le Routeur doit lire ces données en < 1ms sans toucher Postgres.

#### Pattern: Set & Hash
*   **Discovery Set**: Liste des instances actives pour un modèle donné.
    *   Key: `catalog:model:{model_id}:instances` (Set)
    *   Value: `{instance_id}`

*   **Instance State**: Détails techniques pour le routing.
    *   Key: `instance:{instance_id}` (Hash)
    *   Fields:
        *   `ip`: "192.168.1.10"
        *   `port`: "8000"
        *   `status`: "READY"
        *   `current_load`: "12" (Nombre de requêtes actives - mis à jour par le router/worker)
        *   `last_heartbeat`: Timestamp (pour expiration automatique)

## 4. CQRS & Event-Driven Architecture (v0.3.2)
L'architecture a évolué pour séparer strictement les responsabilités (CQRS) et isoler l'Orchestrateur.

### A. Flux de Données
*   **Frontend**: Ne communique **jamais** avec l'Orchestrateur. Il parle uniquement au Backend (Gateway).
*   **Backend (Product Plane)**:
    *   **Read Model**: Lit directement la DB (Postgres) pour les requêtes (GET /instances).
    *   **Write Model**: Valide les requêtes et publie des **Commandes** dans Redis (`orchestrator_events`).
*   **Orchestrateur (Control Plane)**:
    *   Agit comme un **Worker**.
    *   Écoute les événements Redis (`CMD:PROVISION`, `CMD:TERMINATE`).
    *   Exécute les opérations IaaS (Scaleway, AWS).
    *   Met à jour la "Vérité Terrain" dans Postgres.

### B. API Contracts & Documentation
Le Backend expose une API documentée via **Swagger/OpenAPI**.
*   URL Locale: `http://localhost:8003/swagger-ui`
*   JSON Spec: `http://localhost:8003/api-docs/openapi.json`

### C. Workflows

#### 4.1. Provisioning (Command)
1.  **User**: `POST /deployments` (Backend).
2.  **Backend**: Publie `CMD:PROVISION` dans Redis. Renvoie `200 Accepted`.
3.  **Orchestrator**: Reçoit `CMD:PROVISION`. Crée l'instance (Scaleway).
4.  **Orchestrator**: INSERT `instances` (Status: Booting) -> DB.
5.  **Frontend**: Polling `GET /instances` -> Voit "Booting".

#### 4.2. Termination (Command)
1.  **User**: `DELETE /instances/:id` (Backend).
2.  **Backend**: Publie `CMD:TERMINATE` dans Redis.
3.  **Orchestrator**: Reçoit `CMD:TERMINATE`. Supprime l'instance (Scaleway).
4.  **Orchestrator**: UPDATE `instances` SET status='Terminated' -> DB.

#### 4.3. Monitoring (Query)
1.  **User**: Dashboard (Frontend).
2.  **Frontend**: `GET /api/backend/instances`.
3.  **Backend**: `SELECT * FROM instances WHERE organization_id = $1` (Postgres) - Filtré selon workspace.

---

## 5. Multi-Tenant Data Model (Vision Cible)

### 5.1 Workspace Scoping

**Règle fondamentale** : Le **workspace (session) actif** détermine le contexte de toutes les opérations métier.

**Exemples** :
- `GET /instances` → Filtre par `organization_id = current_organization_id` si workspace org
- `POST /deployments` → Crée instance avec `organization_id = current_organization_id` si workspace org
- `GET /models` → Filtre par `organization_id = current_organization_id` OU `organization_id IS NULL` (publics)
- `GET /finops/cost/current` → Filtre par `organization_id = current_organization_id` si workspace org

### 5.2 Plan & Wallet selon Workspace

**Plan** :
- Session Personal → `users.account_plan` détermine modèles accessibles
- Session Org A → `organizations.subscription_plan` (org A) détermine modèles accessibles
- Session Org B → `organizations.subscription_plan` (org B) détermine modèles accessibles

**Wallet** :
- Session Personal → Débit depuis `users.wallet_balance_eur`
- Session Org A → Débit depuis `organizations.wallet_balance_eur` (org A)
- Session Org B → Débit depuis `organizations.wallet_balance_eur` (org B)

### 5.3 RBAC selon Rôle Organisation

**Permissions par rôle** (voir `docs/RBAC_ANALYSIS.md` pour détails) :
- **Owner** : Toutes les permissions (mais doit faire double activation explicitement)
- **Admin** : Gestion technique (instances, models, infrastructure, activation tech)
- **Manager** : Gestion financière (prix, autorisations, dashboards, activation eco)
- **User** : Utilisation des ressources (lecture seule sur instances/models)

### 5.4 Double Activation (Tech + Eco)

**Règle** : Une ressource (instance, model, API key, etc.) est **opérationnelle** uniquement si :
- `tech_activated_by IS NOT NULL` (activation technique par Admin/Owner)
- `eco_activated_by IS NOT NULL` (activation économique par Manager/Owner)

**Permissions** :
- Owner peut activer tech + eco (mais doit faire les 2 activations explicitement)
- Admin peut activer tech uniquement
- Manager peut activer eco uniquement
- User ne peut rien activer

**UX** : Si une ressource n'est pas opérationnelle, afficher état "non opérationnel" + alerte indiquant le flag manquant.

### 5.5 Model Visibility & Access Policy

**Visibility** :
- `public` : Visible à tous les users (plateforme)
- `unlisted` : Non listé mais accessible via identifiant direct si autorisé
- `private` : Visible uniquement aux membres de l'org provider

**Access Policy** :
- `free` : Usage gratuit
- `subscription_required` : Réservé aux abonnés (plan org ou user selon workspace)
- `request_required` : Demande d'accès + approbation requise
- `pay_per_token` : Facturation au token (débit depuis wallet selon workspace)
- `trial` : Gratuit jusqu'à date/quota

**Résolution** :
- Modèles accessibles = Union de :
  - Modèles org (`organization_id = current_organization_id`) si workspace org
  - Modèles publics (`organization_id IS NULL`) selon plan workspace
  - Modèles partagés (`organization_model_shares` actifs) si workspace org

### 5.6 Index & Performance

**Index recommandés** :
```sql
-- Performance workspace scoping
CREATE INDEX idx_instances_org ON instances(organization_id) WHERE organization_id IS NOT NULL;
CREATE INDEX idx_models_org ON models(organization_id) WHERE organization_id IS NOT NULL;
CREATE INDEX idx_api_keys_org ON api_keys(organization_id) WHERE organization_id IS NOT NULL;

-- Performance RBAC
CREATE INDEX idx_organization_memberships_org_role ON organization_memberships(organization_id, role);
CREATE INDEX idx_organization_memberships_user ON organization_memberships(user_id, organization_id);

-- Performance double activation
CREATE INDEX idx_instances_operational ON instances(organization_id, is_operational) WHERE is_operational = true;
CREATE INDEX idx_models_operational ON models(organization_id, is_operational) WHERE is_operational = true;
```

---

## 6. Migration Strategy

**Approche** : Modèle propre dès le départ (pas de legacy).

**Migrations SQL** :
1. Enrichir `users` avec `account_plan`, `wallet_balance_eur`
2. Enrichir `organizations` avec `subscription_plan`, `wallet_balance_eur`, `sidebar_color`
3. Ajouter `organization_id` aux tables scopées (`instances`, `models`, `api_keys`, etc.)
4. Ajouter colonnes double activation (`tech_activated_by`, `eco_activated_by`, `is_operational`)
5. Créer index de performance

**Données seed** :
- Default admin user (`account_plan = 'free'` par défaut)
- Default organisation "Inventiv IT" (`subscription_plan = 'free'` par défaut)
- Admin user = Owner de "Inventiv IT"

---

## 7. Règles de Cohérence

1. **Workspace = Scope** : Toutes les opérations métier sont scopées selon le workspace actif
2. **Plan selon Workspace** : Le plan (user ou org) s'applique selon le workspace actif
3. **Wallet selon Workspace** : Le wallet (user ou org) s'applique selon le workspace actif
4. **Double Activation** : Owner doit faire les 2 activations explicitement (même s'il a les 2 rôles)
5. **Pas de Legacy** : Modèle propre dès le départ, pas de migration de données legacy
6. **Users First-Class** : Un user sans org reste "first-class" et peut utiliser la plateforme
7. **Multi-Sessions** : Un user peut avoir plusieurs sessions simultanées avec des workspaces différents
