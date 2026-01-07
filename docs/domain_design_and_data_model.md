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
-- users (admins)
CREATE TABLE users (
    id UUID PRIMARY KEY,
    email VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    role VARCHAR(50) DEFAULT 'admin',
    created_at TIMESTAMPTZ DEFAULT NOW()
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

-- instances
CREATE TABLE instances (
    id UUID PRIMARY KEY,
    provider_id UUID REFERENCES providers(id),
    zone_id UUID REFERENCES zones(id),
    instance_type_id UUID REFERENCES instance_types(id),
    
    provider_instance_id VARCHAR(255),  -- ID distant
    ip_address INET,
    
    api_key VARCHAR(255), -- Key to call the worker securely
    
    status VARCHAR(50) NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    terminated_at TIMESTAMPTZ,
    gpu_profile JSONB NOT NULL -- Snapshot des specs
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
3.  **Backend**: `SELECT * FROM instances` (Postgres).
