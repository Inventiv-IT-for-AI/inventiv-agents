# Phase 2 : Scoping Instances par Organisation - Plan d'Implémentation

**Date** : 2025-01-XX  
**Statut** : Prêt à démarrer  
**Prérequis** : Phase 1 complète (Architecture Sessions Multi-Org)

---

## 🎯 Objectifs

1. **Isoler les instances par `organization_id`** : Chaque instance appartient à une organisation (ou est publique)
2. **Scoper selon workspace** : Les instances visibles/modifiables dépendent du workspace actif
3. **RBAC complet** : Permissions selon rôle org (Owner/Admin/Manager/User)
4. **Double activation** : Activation technique (Admin/Owner) + économique (Manager/Owner)
5. **Pas de legacy** : Modèle propre dès le départ (pas de migration de données)

---

## 📋 Règles Validées

### 1. Workspace = Scope
- **Session Personal** → Instances avec `organization_id IS NULL` (instances publiques/legacy - mais pas de legacy donc vide)
- **Session Org A** → Instances avec `organization_id = org_a_id`
- **Session Org B** → Instances avec `organization_id = org_b_id`
- Switch workspace → Le scope change immédiatement

### 2. Plan selon Workspace
- **Session Personal** → `users.account_plan` détermine modèles accessibles
- **Session Org A** → `organizations.subscription_plan` (org A) détermine modèles accessibles
- **Session Org B** → `organizations.subscription_plan` (org B) détermine modèles accessibles

### 3. Wallet selon Workspace
- **Session Personal** → Débit depuis `users.wallet_balance_eur`
- **Session Org A** → Débit depuis `organizations.wallet_balance_eur` (org A)
- **Session Org B** → Débit depuis `organizations.wallet_balance_eur` (org B)

### 4. Double Activation
- Owner peut activer tech + eco (mais doit faire les 2 activations explicitement)
- Admin peut activer tech uniquement
- Manager peut activer eco uniquement
- User ne peut rien activer
- Ressource opérationnelle uniquement si `tech_activated_by IS NOT NULL AND eco_activated_by IS NOT NULL`

### 5. Pas de Legacy
- Modèle propre dès le départ
- Pas de migration de données legacy
- Seulement migrations SQL (ajout colonnes, contraintes)

---

## 🔧 Implémentation

### Étape 1 : Migration SQL - Enrichir Data Model

#### 1.1 Ajouter `account_plan` et `wallet_balance_eur` à `users`

**Migration** : `202501XX00001_add_user_account_plan_and_wallet.sql`

```sql
-- Ajouter account_plan et wallet à users
ALTER TABLE users 
  ADD COLUMN IF NOT EXISTS account_plan TEXT DEFAULT 'free' NOT NULL,
  ADD COLUMN IF NOT EXISTS account_plan_updated_at TIMESTAMPTZ,
  ADD COLUMN IF NOT EXISTS wallet_balance_eur NUMERIC(10,2) DEFAULT 0 NOT NULL;

ALTER TABLE users 
  ADD CONSTRAINT users_account_plan_check CHECK (account_plan IN ('free', 'subscriber'));

-- Index pour performance
CREATE INDEX IF NOT EXISTS idx_users_account_plan ON users(account_plan) WHERE account_plan = 'subscriber';
```

#### 1.2 Ajouter `subscription_plan` et `wallet_balance_eur` à `organizations`

**Migration** : `202501XX00002_add_org_subscription_plan_and_wallet.sql`

```sql
-- Ajouter subscription_plan, wallet et sidebar_color à organizations
ALTER TABLE organizations 
  ADD COLUMN IF NOT EXISTS subscription_plan TEXT DEFAULT 'free' NOT NULL,
  ADD COLUMN IF NOT EXISTS subscription_plan_updated_at TIMESTAMPTZ,
  ADD COLUMN IF NOT EXISTS wallet_balance_eur NUMERIC(10,2) DEFAULT 0 NOT NULL,
  ADD COLUMN IF NOT EXISTS sidebar_color TEXT;

ALTER TABLE organizations 
  ADD CONSTRAINT organizations_subscription_plan_check CHECK (subscription_plan IN ('free', 'subscriber'));

-- Index pour performance
CREATE INDEX IF NOT EXISTS idx_organizations_subscription_plan ON organizations(subscription_plan) WHERE subscription_plan = 'subscriber';
```

#### 1.3 Ajouter `organization_id` à `instances`

**Migration** : `202501XX00003_add_instances_organization_id.sql`

```sql
-- Ajouter organization_id à instances
ALTER TABLE instances 
  ADD COLUMN IF NOT EXISTS organization_id UUID REFERENCES organizations(id) ON DELETE SET NULL;

-- Index pour performance (workspace scoping)
CREATE INDEX IF NOT EXISTS idx_instances_org ON instances(organization_id) WHERE organization_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_instances_org_status ON instances(organization_id, status) WHERE organization_id IS NOT NULL;
```

#### 1.4 Ajouter double activation à `instances`

**Migration** : `202501XX00004_add_instances_double_activation.sql`

```sql
-- Ajouter colonnes double activation à instances
ALTER TABLE instances 
  ADD COLUMN IF NOT EXISTS tech_activated_by UUID REFERENCES users(id),
  ADD COLUMN IF NOT EXISTS tech_activated_at TIMESTAMPTZ,
  ADD COLUMN IF NOT EXISTS eco_activated_by UUID REFERENCES users(id),
  ADD COLUMN IF NOT EXISTS eco_activated_at TIMESTAMPTZ;

-- Colonne calculée is_operational
ALTER TABLE instances 
  ADD COLUMN IF NOT EXISTS is_operational BOOLEAN GENERATED ALWAYS AS (
    tech_activated_by IS NOT NULL AND eco_activated_by IS NOT NULL
  ) STORED;

-- Index pour performance
CREATE INDEX IF NOT EXISTS idx_instances_operational ON instances(organization_id, is_operational) WHERE is_operational = true;
CREATE INDEX IF NOT EXISTS idx_instances_tech_activation ON instances(organization_id, tech_activated_by) WHERE tech_activated_by IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_instances_eco_activation ON instances(organization_id, eco_activated_by) WHERE eco_activated_by IS NOT NULL;
```

---

### Étape 2 : Backend Rust - Enrichir Types et Helpers

#### 2.1 Créer helpers pour résoudre plan/wallet selon workspace

**Fichier** : `inventiv-api/src/organizations.rs` (nouveau module ou extension)

```rust
/// Résoudre le plan actif selon le workspace (session)
pub async fn resolve_active_plan(
    db: &Pool<Postgres>,
    user_id: uuid::Uuid,
    current_organization_id: Option<uuid::Uuid>,
) -> anyhow::Result<String> {
    if let Some(org_id) = current_organization_id {
        // Workspace org → plan org
        let plan: Option<String> = sqlx::query_scalar(
            "SELECT subscription_plan FROM organizations WHERE id = $1"
        )
        .bind(org_id)
        .fetch_optional(db)
        .await?;
        Ok(plan.unwrap_or_else(|| "free".to_string()))
    } else {
        // Workspace personal → plan user
        let plan: Option<String> = sqlx::query_scalar(
            "SELECT account_plan FROM users WHERE id = $1"
        )
        .bind(user_id)
        .fetch_optional(db)
        .await?;
        Ok(plan.unwrap_or_else(|| "free".to_string()))
    }
}

/// Résoudre le wallet actif selon le workspace (session)
pub async fn resolve_active_wallet(
    db: &Pool<Postgres>,
    user_id: uuid::Uuid,
    current_organization_id: Option<uuid::Uuid>,
) -> anyhow::Result<Option<rust_decimal::Decimal>> {
    if let Some(org_id) = current_organization_id {
        // Workspace org → wallet org
        let balance: Option<rust_decimal::Decimal> = sqlx::query_scalar(
            "SELECT wallet_balance_eur FROM organizations WHERE id = $1"
        )
        .bind(org_id)
        .fetch_optional(db)
        .await?;
        Ok(balance)
    } else {
        // Workspace personal → wallet user
        let balance: Option<rust_decimal::Decimal> = sqlx::query_scalar(
            "SELECT wallet_balance_eur FROM users WHERE id = $1"
        )
        .bind(user_id)
        .fetch_optional(db)
        .await?;
        Ok(balance)
    }
}
```

#### 2.2 Étendre RBAC avec permissions instances

**Fichier** : `inventiv-api/src/rbac.rs`

```rust
/// Vérifier si un rôle peut voir les instances
pub fn can_view_instances(role: &OrgRole) -> bool {
    matches!(role, OrgRole::Owner | OrgRole::Admin | OrgRole::Manager | OrgRole::User)
}

/// Vérifier si un rôle peut créer/modifier/terminer instances
pub fn can_modify_instances(role: &OrgRole) -> bool {
    matches!(role, OrgRole::Owner | OrgRole::Admin)
}

/// Vérifier si un rôle peut activer techniquement
pub fn can_activate_tech(role: &OrgRole) -> bool {
    matches!(role, OrgRole::Owner | OrgRole::Admin)
}

/// Vérifier si un rôle peut activer économiquement
pub fn can_activate_eco(role: &OrgRole) -> bool {
    matches!(role, OrgRole::Owner | OrgRole::Manager)
}
```

#### 2.3 Modifier `list_instances()` pour scoper selon workspace

**Fichier** : `inventiv-api/src/handlers/instances.rs`

```rust
pub async fn list_instances(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<auth::AuthUser>,
) -> impl IntoResponse {
    // Scoper selon workspace
    let query = if let Some(org_id) = user.current_organization_id {
        // Workspace org → seulement instances org
        sqlx::query_as!(
            InstanceRow,
            r#"
            SELECT id, provider_id, zone_id, instance_type_id, organization_id,
                   provider_instance_id, ip_address, status, 
                   tech_activated_by, tech_activated_at, eco_activated_by, eco_activated_at, is_operational,
                   created_at, terminated_at, gpu_profile
            FROM instances
            WHERE organization_id = $1
            ORDER BY created_at DESC
            "#,
            org_id
        )
    } else {
        // Workspace personal → pas d'instances (modèle propre, pas de legacy)
        sqlx::query_as!(
            InstanceRow,
            r#"
            SELECT id, provider_id, zone_id, instance_type_id, organization_id,
                   provider_instance_id, ip_address, status,
                   tech_activated_by, tech_activated_at, eco_activated_by, eco_activated_at, is_operational,
                   created_at, terminated_at, gpu_profile
            FROM instances
            WHERE 1 = 0  -- Pas d'instances en mode personal
            ORDER BY created_at DESC
            "#
        )
    };
    
    // ... reste du code
}
```

#### 2.4 Modifier `create_deployment()` pour définir `organization_id`

**Fichier** : `inventiv-api/src/handlers/deployments.rs`

```rust
pub async fn create_deployment(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<auth::AuthUser>,
    Json(req): Json<CreateDeploymentRequest>,
) -> impl IntoResponse {
    // Vérifier RBAC : seulement Admin/Owner peuvent créer instances
    if let Some(org_id) = user.current_organization_id {
        let Some(org_role) = &user.current_organization_role else {
            return Err(StatusCode::FORBIDDEN);
        };
        let role = rbac::OrgRole::parse(org_role)?;
        if !rbac::can_modify_instances(&role) {
            return Err(StatusCode::FORBIDDEN);
        }
    } else {
        // Mode personal → pas d'instances (org requis)
        return Err(StatusCode::BAD_REQUEST);
    }
    
    // Créer instance avec organization_id
    let org_id = user.current_organization_id.unwrap();
    
    // Publier CMD:PROVISION avec organization_id dans metadata
    // ... reste du code
}
```

#### 2.5 Ajouter endpoints pour double activation

**Fichier** : `inventiv-api/src/handlers/instances.rs`

```rust
/// Activer techniquement une instance
pub async fn activate_instance_tech(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<auth::AuthUser>,
    Path(instance_id): Path<uuid::Uuid>,
) -> impl IntoResponse {
    // Vérifier workspace org
    let Some(org_id) = user.current_organization_id else {
        return Err(StatusCode::BAD_REQUEST);
    };
    
    // Vérifier RBAC : Admin/Owner uniquement
    let Some(org_role) = &user.current_organization_role else {
        return Err(StatusCode::FORBIDDEN);
    };
    let role = rbac::OrgRole::parse(org_role)?;
    if !rbac::can_activate_tech(&role) {
        return Err(StatusCode::FORBIDDEN);
    }
    
    // Vérifier que l'instance appartient à l'org
    let instance_org_id: Option<uuid::Uuid> = sqlx::query_scalar(
        "SELECT organization_id FROM instances WHERE id = $1"
    )
    .bind(instance_id)
    .fetch_optional(&state.db)
    .await?
    .flatten();
    
    if instance_org_id != Some(org_id) {
        return Err(StatusCode::FORBIDDEN);
    }
    
    // Activer tech
    sqlx::query(
        r#"
        UPDATE instances
        SET tech_activated_by = $1,
            tech_activated_at = NOW()
        WHERE id = $2
        "#
    )
    .bind(user.user_id)
    .bind(instance_id)
    .execute(&state.db)
    .await?;
    
    Ok(Json(json!({"status":"ok"})))
}

/// Activer économiquement une instance
pub async fn activate_instance_eco(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<auth::AuthUser>,
    Path(instance_id): Path<uuid::Uuid>,
) -> impl IntoResponse {
    // Même logique mais avec can_activate_eco
    // ... code similaire
}
```

---

### Étape 3 : Frontend - Badges, Filtres, Visibilité

#### 3.1 Ajouter badges workspace sur page instances

**Fichier** : `inventiv-frontend/src/app/(app)/instances/page.tsx`

```typescript
export default function InstancesPage() {
  const { me } = useAuth();  // Hook à créer ou utiliser existant
  const { orgRole, hasOrg } = useOrgRole();  // Hook à créer
  const { can } = useCan();  // Hook à créer
  
  return (
    <div>
      {/* Workspace Banner */}
      <WorkspaceBanner />
      
      {/* Badge workspace */}
      {hasOrg && (
        <Badge variant="secondary">
          {me?.current_organization_name || "Organisation"}
        </Badge>
      )}
      
      {/* Bouton créer instance (seulement Admin/Owner) */}
      {hasOrg && can('instances.create') && (
        <Button onClick={handleCreate}>Créer une instance</Button>
      )}
      
      {/* Liste instances */}
      <InstancesTable 
        instances={instances}
        onTerminate={can('instances.modify') ? handleTerminate : undefined}
        onReinstall={can('instances.modify') ? handleReinstall : undefined}
        onActivateTech={can('instances.activate_tech') ? handleActivateTech : undefined}
        onActivateEco={can('instances.activate_eco') ? handleActivateEco : undefined}
      />
    </div>
  );
}
```

#### 3.2 Afficher état opérationnel dans table instances

**Fichier** : `inventiv-frontend/src/components/instances/InstancesTable.tsx`

```typescript
// Colonne "État opérationnel"
{is_operational ? (
  <Badge variant="success">Opérationnel</Badge>
) : (
  <Badge variant="warning">
    {!tech_activated_by && !eco_activated_by ? "Non activé" : 
     !tech_activated_by ? "Activation tech requise" :
     "Activation eco requise"}
  </Badge>
)}
```

---

### Étape 4 : Tests

#### 4.1 Tests unitaires RBAC

**Fichier** : `inventiv-api/src/rbac.rs` (tests)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_can_modify_instances() {
        assert!(can_modify_instances(&OrgRole::Owner));
        assert!(can_modify_instances(&OrgRole::Admin));
        assert!(!can_modify_instances(&OrgRole::Manager));
        assert!(!can_modify_instances(&OrgRole::User));
    }
    
    #[test]
    fn test_can_activate_tech() {
        assert!(can_activate_tech(&OrgRole::Owner));
        assert!(can_activate_tech(&OrgRole::Admin));
        assert!(!can_activate_tech(&OrgRole::Manager));
        assert!(!can_activate_tech(&OrgRole::User));
    }
    
    #[test]
    fn test_can_activate_eco() {
        assert!(can_activate_eco(&OrgRole::Owner));
        assert!(!can_activate_eco(&OrgRole::Admin));
        assert!(can_activate_eco(&OrgRole::Manager));
        assert!(!can_activate_eco(&OrgRole::User));
    }
}
```

#### 4.2 Tests d'intégration scoping

**Fichier** : `inventiv-api/tests/integration/instances_scoping.rs` (nouveau)

```rust
#[tokio::test]
async fn test_list_instances_scoped_by_org() {
    // Créer org A et org B
    // Créer instances pour org A et org B
    // Login avec session org A
    // Vérifier que seulement instances org A sont retournées
    // Switch vers org B
    // Vérifier que seulement instances org B sont retournées
}

#[tokio::test]
async fn test_create_instance_sets_organization_id() {
    // Login avec session org A
    // Créer instance
    // Vérifier que instance.organization_id = org A
}

#[tokio::test]
async fn test_user_cannot_create_instance() {
    // Login avec session org A, rôle User
    // Tenter créer instance
    // Vérifier 403 Forbidden
}
```

#### 4.3 Tests manuels

- [ ] Mode Personal → Vérifier que page instances est vide (ou masquée)
- [ ] Mode Org User → Vérifier que instances sont visibles mais boutons créer/modifier masqués
- [ ] Mode Org Admin → Vérifier que instances sont visibles et modifiables
- [ ] Mode Org Manager → Vérifier que instances sont visibles, activation eco possible
- [ ] Switch workspace → Vérifier que liste instances change immédiatement
- [ ] Double activation → Vérifier que ressource non opérationnelle si un flag manque

---

## 📊 Checklist Complète

### Migrations SQL
- [ ] Migration `add_user_account_plan_and_wallet.sql`
- [ ] Migration `add_org_subscription_plan_and_wallet.sql`
- [ ] Migration `add_instances_organization_id.sql`
- [ ] Migration `add_instances_double_activation.sql`
- [ ] Vérifier index créés
- [ ] Tester migrations sur DB de test

### Backend Rust
- [ ] Helpers `resolve_active_plan()` et `resolve_active_wallet()`
- [ ] Étendre RBAC avec permissions instances
- [ ] Modifier `list_instances()` pour scoper selon workspace
- [ ] Modifier `create_deployment()` pour définir `organization_id`
- [ ] Modifier `get_instance()`, `terminate_instance()`, `reinstall_instance()` pour vérifier RBAC
- [ ] Endpoints `activate_instance_tech()` et `activate_instance_eco()`
- [ ] Tests unitaires RBAC
- [ ] Tests d'intégration scoping

### Frontend
- [ ] `WorkspaceBanner` visible sur page instances
- [ ] Badge workspace sur page instances
- [ ] Masquer boutons selon rôle org
- [ ] Colonne "État opérationnel" dans table instances
- [ ] Boutons activation tech/eco selon rôle
- [ ] Alerte si ressource non opérationnelle

### Tests
- [ ] Tests unitaires RBAC
- [ ] Tests d'intégration scoping
- [ ] Tests manuels (mode Personal + modes Org)

---

## 🎯 Estimation

**Temps total** : 6-8h développement + 2-3h tests

**Répartition** :
- Migrations SQL : 1h
- Backend Rust : 4-5h
- Frontend : 1-2h
- Tests : 2-3h

---

**Prochaine étape** : Valider ce plan et commencer par les migrations SQL.

