# Analyse Détaillée : Gestion des Rôles dans les Organisations

## Objectif
Analyser l'état actuel et définir les besoins pour la gestion complète des rôles utilisateurs dans les organisations, conditionnant l'affichage des modules et fonctions selon le rôle.

---

## 1. État Actuel

### 1.1 Base de Données (DB)

#### Tables existantes ✅
- **`organizations`** : Organisations créées
- **`organization_memberships`** : 
  - Colonnes : `organization_id`, `user_id`, `role` (CHECK: `owner|admin|manager|user`)
  - Contrainte CHECK : `role IN ('owner', 'admin', 'manager', 'user')`
  - Index : `organization_memberships_user_idx` sur `(user_id, organization_id)`
- **`users`** : 
  - Colonne : `current_organization_id` (nullable) → workspace courant
  - Colonne : `role` (user role global : `admin|user`) → **⚠️ À distinguer du rôle org**

#### Ce qui manque ⏳
- **PRIMARY KEY** sur `organization_memberships` → **CRITIQUE** (Phase 0 migration)
- **FOREIGN KEY** vers `organizations` et `users` → **CRITIQUE**
- **Index** sur `(organization_id, role)` pour requêtes filtrées par rôle
- **Colonne `role` dans JWT** → actuellement seul `current_organization_id` est dans le JWT

---

### 1.2 API Backend (Rust)

#### Module RBAC existant ✅ (`inventiv-api/src/rbac.rs`)
- **`OrgRole` enum** : `Owner`, `Admin`, `Manager`, `User`
- **Fonctions de permission** :
  - `can_invite(role)` → Owner/Admin/Manager peuvent inviter
  - `can_set_activation_flag(role, flag)` → Owner (tech+eco), Admin (tech), Manager (eco)
  - `can_assign_role(actor, from, to)` → règles de délégation

#### Endpoints existants ✅ (`inventiv-api/src/organizations.rs`)
- `GET /organizations/current/members` → Liste membres avec rôles
- `PUT /organizations/current/members/:user_id` → Changer rôle (avec RBAC check)
- `DELETE /organizations/current/members/:user_id` → Retirer membre (avec RBAC check)
- `POST /organizations/current/leave` → Quitter org

#### Helpers existants ✅
- `get_membership_role(db, org_id, user_id)` → Récupère le rôle d'un user dans une org
- `is_member(db, org_id, user_id)` → Vérifie si user est membre

#### Ce qui manque ⏳

**1. Rôle dans le JWT/AuthUser**
- `AuthUser` contient `current_organization_id` mais **pas `current_organization_role`**
- Le rôle doit être **résolu à chaque requête** depuis la DB (coût performance)
- **Solution** : Ajouter `current_organization_role: Option<String>` dans `AuthUser` et JWT

**2. Middleware RBAC réutilisable**
- Pas de middleware générique pour vérifier le rôle org
- Chaque endpoint fait sa propre vérification manuelle
- **Solution** : Créer `require_org_role(role: OrgRole)` middleware

**3. Fonctions de permission manquantes**
- `can_view_instances(role)` → Qui peut voir/gérer les instances
- `can_view_models(role)` → Qui peut voir/gérer les modèles
- `can_view_users(role)` → Qui peut voir/gérer les users
- `can_view_settings(role)` → Qui peut voir/gérer les settings
- `can_view_finops(role)` → Qui peut voir les dashboards financiers
- `can_modify_instances(role)` → Qui peut créer/modifier/terminer instances
- `can_modify_models(role)` → Qui peut créer/modifier/supprimer modèles
- `can_modify_users(role)` → Qui peut créer/modifier/supprimer users
- `can_modify_settings(role)` → Qui peut modifier settings infrastructure

**4. Endpoint pour récupérer le rôle courant**
- `GET /organizations/current/role` → Retourne le rôle du user dans l'org courante
- Utile pour le Frontend pour décider quoi afficher

---

### 1.3 Frontend (Next.js/React)

#### État actuel ✅
- **`Sidebar.tsx`** : Affiche liens selon `meRole` (user global role `admin|user`)
- **`AccountSection.tsx`** : Gère workspace (Personal vs Org)
- **`OrganizationMembersDialog.tsx`** : Affiche membres avec rôles, permet changement rôle

#### Ce qui manque ⏳

**1. Rôle org dans le state Frontend**
- `Me` type contient `current_organization_id` mais **pas `current_organization_role`**
- Le Frontend ne sait pas quel rôle le user a dans l'org courante
- **Solution** : Ajouter `current_organization_role?: string | null` dans `Me` type

**2. Hooks RBAC réutilisables**
- Pas de hook `useOrgRole()` pour récupérer le rôle courant
- Pas de hook `useCan(permission)` pour vérifier permissions
- **Solution** : Créer hooks React pour RBAC

**3. Affichage conditionnel selon rôle**
- Sidebar affiche tous les liens (sauf Settings/Users si `role !== 'admin'`)
- Pas de masquage selon rôle **org** (Owner/Admin/Manager/User)
- **Solution** : Masquer liens selon rôle org + workspace

**4. Badges/Indicateurs visuels**
- Pas d'indication visuelle du rôle org courant
- Pas de badges "Owner", "Admin", "Manager" dans l'UI
- **Solution** : Afficher badge rôle dans Sidebar/Header

**5. Désactivation d'actions selon rôle**
- Boutons "Créer", "Modifier", "Supprimer" visibles même si user n'a pas les droits
- **Solution** : Désactiver/masquer boutons selon permissions RBAC

---

## 2. Matrice de Permissions par Rôle et par Module

### 2.1 Définition des Modules

| Module | Description | Workspace Requis |
|--------|-------------|------------------|
| **Chat** | Chat avec LLMs | Personal ou Org |
| **Workbench** | Sessions/projets de travail | Personal ou Org |
| **API Keys** | Gestion clés API | Personal ou Org |
| **Instances** | Provisioning/gestion instances GPU | **Org requis** |
| **Models** | Catalogue modèles | Personal ou Org (voir publics + org) |
| **Users** | Gestion users/membres | **Org requis** |
| **Settings** | Infrastructure (providers/regions/zones/types) | **Org requis** |
| **FinOps** | Dashboards coûts/dépenses | Personal ou Org |
| **Observability** | Métriques GPU/system | **Org requis** |
| **Monitoring** | Logs/events | **Org requis** |

### 2.2 Matrice de Permissions Détaillée

#### Module : **Instances** (Org requis)

| Action | Owner | Admin | Manager | User | Notes |
|--------|-------|-------|---------|------|-------|
| **Voir instances** | ✅ | ✅ | ✅ | ✅ | Tous peuvent voir |
| **Créer instance** | ✅ | ✅ | ❌ | ❌ | Admin/Owner uniquement |
| **Modifier instance** | ✅ | ✅ | ❌ | ❌ | Admin/Owner uniquement |
| **Terminer instance** | ✅ | ✅ | ❌ | ❌ | Admin/Owner uniquement |
| **Réinstaller instance** | ✅ | ✅ | ❌ | ❌ | Admin/Owner uniquement |
| **Voir métriques** | ✅ | ✅ | ✅ | ✅ | Tous peuvent voir |
| **Activer tech** | ✅ | ✅ | ❌ | ❌ | Admin/Owner uniquement |
| **Activer eco** | ✅ | ❌ | ✅ | ❌ | Manager/Owner uniquement |

#### Module : **Models** (Personal ou Org)

| Action | Owner | Admin | Manager | User | Notes |
|--------|-------|-------|---------|------|-------|
| **Voir modèles publics** | ✅ | ✅ | ✅ | ✅ | Tous peuvent voir |
| **Voir modèles org** | ✅ | ✅ | ✅ | ✅ | Membres org peuvent voir |
| **Créer modèle** | ✅ | ✅ | ❌ | ❌ | Admin/Owner uniquement |
| **Modifier modèle** | ✅ | ✅ | ❌ | ❌ | Admin/Owner uniquement |
| **Supprimer modèle** | ✅ | ✅ | ❌ | ❌ | Admin/Owner uniquement |
| **Publier offering** | ✅ | ✅ | ❌ | ❌ | Admin/Owner uniquement |
| **Activer tech** | ✅ | ✅ | ❌ | ❌ | Admin/Owner uniquement |
| **Activer eco** | ✅ | ❌ | ✅ | ❌ | Manager/Owner uniquement |

#### Module : **Users/Members** (Org requis)

| Action | Owner | Admin | Manager | User | Notes |
|--------|-------|-------|---------|------|-------|
| **Voir membres** | ✅ | ✅ | ✅ | ✅ | Tous peuvent voir |
| **Inviter user** | ✅ | ✅ | ✅ | ❌ | Owner/Admin/Manager |
| **Changer rôle** | ✅ | ⚠️ | ⚠️ | ❌ | Owner (tout), Admin (Admin↔User), Manager (Manager↔User) |
| **Retirer membre** | ✅ | ⚠️ | ⚠️ | ⚠️ | Owner (tout), Admin (Admin/User), Manager (Manager/User), User (soi-même) |
| **Créer user** | ✅ | ✅ | ❌ | ❌ | Admin/Owner uniquement |

#### Module : **Settings** (Org requis)

| Action | Owner | Admin | Manager | User | Notes |
|--------|-------|-------|---------|------|-------|
| **Voir settings** | ✅ | ✅ | ✅ | ❌ | Owner/Admin/Manager |
| **Modifier providers** | ✅ | ✅ | ❌ | ❌ | Admin/Owner uniquement |
| **Modifier regions/zones** | ✅ | ✅ | ❌ | ❌ | Admin/Owner uniquement |
| **Modifier instance types** | ✅ | ✅ | ❌ | ❌ | Admin/Owner uniquement |
| **Modifier provider settings** | ✅ | ✅ | ❌ | ❌ | Admin/Owner uniquement |

#### Module : **FinOps** (Personal ou Org)

| Action | Owner | Admin | Manager | User | Notes |
|--------|-------|-------|---------|------|-------|
| **Voir dashboards** | ✅ | ✅ | ✅ | ❌ | Owner/Admin/Manager |
| **Voir coûts** | ✅ | ✅ | ✅ | ❌ | Owner/Admin/Manager |
| **Modifier prix** | ✅ | ❌ | ✅ | ❌ | Manager/Owner uniquement |
| **Autoriser conso** | ✅ | ❌ | ✅ | ❌ | Manager/Owner uniquement |

#### Module : **API Keys** (Personal ou Org)

| Action | Owner | Admin | Manager | User | Notes |
|--------|-------|-------|---------|------|-------|
| **Voir clés user-owned** | ✅ | ✅ | ✅ | ✅ | Tous peuvent voir leurs clés |
| **Voir clés org-owned** | ✅ | ✅ | ✅ | ✅ | Membres org peuvent voir |
| **Créer clé user-owned** | ✅ | ✅ | ✅ | ✅ | Tous peuvent créer |
| **Créer clé org-owned** | ✅ | ✅ | ❌ | ❌ | Admin/Owner uniquement |
| **Modifier clé org-owned** | ✅ | ✅ | ❌ | ❌ | Admin/Owner uniquement |
| **Révoquer clé org-owned** | ✅ | ✅ | ❌ | ❌ | Admin/Owner uniquement |

#### Module : **Chat** (Personal ou Org)

| Action | Owner | Admin | Manager | User | Notes |
|--------|-------|-------|---------|------|-------|
| **Utiliser chat** | ✅ | ✅ | ✅ | ✅ | Tous peuvent chatter |
| **Voir historique** | ✅ | ✅ | ✅ | ✅ | Tous peuvent voir leur historique |
| **Partager session org** | ✅ | ✅ | ✅ | ✅ | Tous peuvent partager |

#### Module : **Workbench** (Personal ou Org)

| Action | Owner | Admin | Manager | User | Notes |
|--------|-------|-------|---------|------|-------|
| **Créer session** | ✅ | ✅ | ✅ | ✅ | Tous peuvent créer |
| **Voir sessions** | ✅ | ✅ | ✅ | ✅ | Tous peuvent voir leurs sessions |
| **Partager session org** | ✅ | ✅ | ✅ | ✅ | Tous peuvent partager |
| **Supprimer session** | ✅ | ✅ | ✅ | ✅ | Tous peuvent supprimer leurs sessions |

---

## 3. Plan d'Implémentation Détaillé

### Phase A : Enrichir AuthUser avec Rôle Org (Backend)

#### 3.1 Migration SQL
```sql
-- Aucune migration nécessaire (rôle déjà dans organization_memberships)
-- Mais on peut ajouter un index pour performance :
CREATE INDEX IF NOT EXISTS idx_organization_memberships_org_role 
ON organization_memberships(organization_id, role) 
WHERE organization_id IS NOT NULL;
```

#### 3.2 Modifier `AuthUser` struct
**Fichier** : `inventiv-api/src/auth.rs`

**Changements** :
```rust
pub struct AuthUser {
    pub user_id: uuid::Uuid,
    pub email: String,
    pub role: String,  // User global role (admin|user)
    pub current_organization_id: Option<uuid::Uuid>,
    // AJOUTER :
    pub current_organization_role: Option<String>,  // Org role (owner|admin|manager|user)
}
```

#### 3.3 Modifier JWT Claims
**Fichier** : `inventiv-api/src/auth.rs`

**Changements** :
- Ajouter `current_organization_role` dans les claims JWT
- Lors du login/switch org → résoudre le rôle depuis DB et l'inclure dans le JWT

#### 3.4 Helper pour résoudre le rôle
**Fichier** : `inventiv-api/src/organizations.rs`

**Changements** :
- Créer fonction `resolve_user_org_role(db, org_id, user_id) -> Option<OrgRole>`
- Utiliser cette fonction lors du login/switch org pour enrichir le JWT

---

### Phase B : Middleware RBAC Réutilisable (Backend)

#### 3.5 Créer middleware `require_org_role`
**Fichier** : `inventiv-api/src/rbac.rs` (nouveau module ou extension)

**Changements** :
```rust
pub async fn require_org_role(
    required_role: OrgRole,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    let Some(user) = req.extensions().get::<auth::AuthUser>() else {
        return (StatusCode::UNAUTHORIZED, ...).into_response();
    };
    
    let Some(org_id) = user.current_organization_id else {
        return (StatusCode::BAD_REQUEST, ...).into_response();
    };
    
    let Some(user_role) = user.current_organization_role
        .and_then(|r| OrgRole::parse(&r)) else {
        return (StatusCode::FORBIDDEN, ...).into_response();
    };
    
    // Vérifier hiérarchie : Owner > Admin > Manager > User
    let has_permission = match (required_role, user_role) {
        (OrgRole::Owner, OrgRole::Owner) => true,
        (OrgRole::Admin, OrgRole::Owner | OrgRole::Admin) => true,
        (OrgRole::Manager, OrgRole::Owner | OrgRole::Admin | OrgRole::Manager) => true,
        (OrgRole::User, _) => true,  // Tous peuvent faire ce que User peut faire
        _ => false,
    };
    
    if !has_permission {
        return (StatusCode::FORBIDDEN, ...).into_response();
    }
    
    next.run(req).await
}
```

#### 3.6 Étendre fonctions de permission
**Fichier** : `inventiv-api/src/rbac.rs`

**Changements** :
- Ajouter toutes les fonctions `can_*` manquantes (voir section 1.2.3)

---

### Phase C : Endpoints pour Rôle Courant (Backend)

#### 3.7 Endpoint `GET /organizations/current/role`
**Fichier** : `inventiv-api/src/organizations.rs`

**Changements** :
```rust
pub async fn get_current_organization_role(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<auth::AuthUser>,
) -> impl IntoResponse {
    let Some(org_id) = user.current_organization_id else {
        return (StatusCode::BAD_REQUEST, Json(json!({"error":"no_current_organization"}))).into_response();
    };
    
    let Some(role) = get_membership_role(&state.db, org_id, user.user_id).await else {
        return (StatusCode::FORBIDDEN, Json(json!({"error":"not_a_member"}))).into_response();
    };
    
    Json(json!({
        "organization_id": org_id,
        "role": role.as_str(),
        "permissions": {
            "can_invite": can_invite(role),
            "can_manage_instances": can_manage_instances(role),
            // ... autres permissions
        }
    })).into_response()
}
```

---

### Phase D : Enrichir Frontend avec Rôle Org

#### 3.8 Modifier type `Me`
**Fichier** : `inventiv-frontend/src/lib/types.ts`

**Changements** :
```typescript
export type Me = {
  // ... champs existants
  current_organization_id?: string | null;
  current_organization_role?: string | null;  // AJOUTER
  current_organization_name?: string | null;
  current_organization_slug?: string | null;
};
```

#### 3.9 Modifier endpoint `/auth/me`
**Fichier** : `inventiv-api/src/auth_endpoints.rs`

**Changements** :
- Lors de `GET /auth/me`, résoudre le rôle org si `current_organization_id` existe
- Inclure `current_organization_role` dans la réponse

#### 3.10 Créer hooks RBAC
**Fichier** : `inventiv-frontend/src/hooks/useOrgRole.ts` (nouveau)

**Changements** :
```typescript
export function useOrgRole() {
  const { me } = useAuth();  // Hook existant ou à créer
  
  const orgRole = me?.current_organization_role 
    ? (me.current_organization_role as 'owner' | 'admin' | 'manager' | 'user')
    : null;
  
  const isOwner = orgRole === 'owner';
  const isAdmin = orgRole === 'admin' || isOwner;
  const isManager = orgRole === 'manager' || isAdmin;
  const isUser = orgRole === 'user' || isManager;
  
  return {
    orgRole,
    isOwner,
    isAdmin,
    isManager,
    isUser,
    hasOrg: !!me?.current_organization_id,
  };
}
```

**Fichier** : `inventiv-frontend/src/hooks/useCan.ts` (nouveau)

**Changements** :
```typescript
export function useCan(permission: string) {
  const { orgRole, hasOrg } = useOrgRole();
  
  // Matrice de permissions (voir section 2.2)
  const permissions: Record<string, ('owner' | 'admin' | 'manager' | 'user')[]> = {
    'instances.create': ['owner', 'admin'],
    'instances.modify': ['owner', 'admin'],
    'models.create': ['owner', 'admin'],
    'users.invite': ['owner', 'admin', 'manager'],
    'settings.modify': ['owner', 'admin'],
    'finops.view': ['owner', 'admin', 'manager'],
    // ... autres permissions
  };
  
  const allowedRoles = permissions[permission] || [];
  return allowedRoles.includes(orgRole as any) && hasOrg;
}
```

---

### Phase E : Affichage Conditionnel Frontend

#### 3.11 Modifier Sidebar
**Fichier** : `inventiv-frontend/src/components/Sidebar.tsx`

**Changements** :
```typescript
export function Sidebar() {
  const { orgRole, hasOrg } = useOrgRole();
  const { can } = useCan();
  
  return (
    <div>
      {/* Modules "For All Users" */}
      <SidebarLink href="/" icon={LayoutDashboard} label="Dashboard" />
      <SidebarLink href="/chat" icon={MessageSquare} label="Chat" />
      <SidebarLink href="/workbench" icon={Terminal} label="Workbench" />
      <SidebarLink href="/api-keys" icon={KeyRound} label="API Keys" />
      
      {/* Modules "Org Required" */}
      {hasOrg && (
        <>
          <SidebarLink href="/instances" icon={Server} label="Instances" />
          <SidebarLink href="/models" icon={Activity} label="Models" />
          <SidebarLink href="/observability" icon={Cpu} label="Observability" />
          
          {/* Modules selon rôle */}
          {can('users.view') && (
            <SidebarLink href="/users" icon={Users} label="Users" />
          )}
          {can('settings.view') && (
            <SidebarLink href="/settings" icon={Settings} label="Settings" />
          )}
          {can('finops.view') && (
            <SidebarLink href="/monitoring" icon={BarChart3} label="Monitoring" />
          )}
        </>
      )}
      
      {/* Badge rôle org */}
      {hasOrg && orgRole && (
        <div className="px-3 py-2">
          <Badge variant="secondary">
            {orgRole.charAt(0).toUpperCase() + orgRole.slice(1)}
          </Badge>
        </div>
      )}
    </div>
  );
}
```

#### 3.12 Modifier pages pour masquer actions
**Exemple** : `inventiv-frontend/src/app/(app)/instances/page.tsx`

**Changements** :
```typescript
export default function InstancesPage() {
  const { can } = useCan();
  
  return (
    <div>
      <WorkspaceBanner />
      
      {/* Bouton créer instance */}
      {can('instances.create') && (
        <Button onClick={handleCreate}>Créer une instance</Button>
      )}
      
      {/* Liste instances */}
      <InstancesTable 
        onTerminate={can('instances.modify') ? handleTerminate : undefined}
        onReinstall={can('instances.modify') ? handleReinstall : undefined}
      />
    </div>
  );
}
```

---

## 4. Checklist d'Implémentation

### Backend
- [ ] Phase A.1 : Ajouter index DB `idx_organization_memberships_org_role`
- [ ] Phase A.2 : Modifier `AuthUser` struct → ajouter `current_organization_role`
- [ ] Phase A.3 : Modifier JWT claims → inclure `current_organization_role`
- [ ] Phase A.4 : Helper `resolve_user_org_role()` dans `organizations.rs`
- [ ] Phase A.5 : Enrichir JWT lors login/switch org
- [ ] Phase B.1 : Créer middleware `require_org_role()`
- [ ] Phase B.2 : Étendre fonctions `can_*` dans `rbac.rs`
- [ ] Phase C.1 : Endpoint `GET /organizations/current/role`
- [ ] Phase C.2 : Modifier `/auth/me` → inclure `current_organization_role`

### Frontend
- [ ] Phase D.1 : Modifier type `Me` → ajouter `current_organization_role`
- [ ] Phase D.2 : Créer hook `useOrgRole()`
- [ ] Phase D.3 : Créer hook `useCan(permission)`
- [ ] Phase E.1 : Modifier `Sidebar.tsx` → masquer liens selon rôle
- [ ] Phase E.2 : Ajouter badge rôle dans Sidebar
- [ ] Phase E.3 : Modifier pages Instances → masquer actions selon rôle
- [ ] Phase E.4 : Modifier pages Models → masquer actions selon rôle
- [ ] Phase E.5 : Modifier pages Users → masquer actions selon rôle
- [ ] Phase E.6 : Modifier pages Settings → masquer selon rôle
- [ ] Phase E.7 : Modifier pages FinOps → masquer selon rôle

### Tests
- [ ] Tests unitaires RBAC (Rust)
- [ ] Tests intégration endpoints avec rôles
- [ ] Tests manuels Frontend (vérifier masquage selon rôle)
- [ ] Tests compatibilité backward (mode Personal fonctionne toujours)

---

## 5. Ordre de Déploiement Recommandé

1. **Phase A** (Backend Auth) → Enrichir JWT avec rôle org
2. **Phase B** (Backend RBAC) → Middleware + fonctions permission
3. **Phase C** (Backend Endpoints) → Endpoint rôle courant
4. **Phase D** (Frontend Hooks) → Hooks RBAC réutilisables
5. **Phase E** (Frontend UI) → Affichage conditionnel

---

## 6. Notes Importantes

### Compatibilité Backward
- Mode Personal (`current_organization_id = NULL`) → `current_organization_role = NULL`
- Les endpoints doivent fonctionner **sans** rôle org (fallback sur user global role si nécessaire)

### Performance
- Rôle dans JWT → évite requête DB à chaque requête
- Cache côté Frontend → éviter appels répétés `/auth/me`

### Sécurité
- **Toujours vérifier côté Backend** → Frontend peut être modifié
- Middleware RBAC → première ligne de défense
- Vérifications supplémentaires dans handlers → deuxième ligne de défense

