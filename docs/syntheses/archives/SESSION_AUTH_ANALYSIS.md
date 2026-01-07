# Analyse : Session Utilisateur et Informations Organisation

## Objectif
Vérifier que les informations suivantes sont présentes et sécurisées dans la session utilisateur :
1. ✅ Informations sur le user connecté
2. ✅ Organisation courante
3. ❌ **Rôle dans cette organisation** → **MANQUANT**
4. ✅ Sécurisé dans les tokens de session (JWT)
5. ✅ Backup côté backend (DB)

---

## 1. État Actuel - Backend (Rust)

### 1.1 Struct `AuthUser` (`inventiv-api/src/auth.rs`)

**État actuel** :
```rust
pub struct AuthUser {
    pub user_id: uuid::Uuid,                    // ✅ Présent
    pub email: String,                          // ✅ Présent
    pub role: String,                           // ✅ Présent (user global role: admin|user)
    pub current_organization_id: Option<uuid::Uuid>,  // ✅ Présent
    // ❌ MANQUE: current_organization_role: Option<String>,
}
```

**Problème** : Le rôle dans l'organisation n'est **pas** dans `AuthUser`, donc il faut le résoudre depuis la DB à chaque requête (performance).

---

### 1.2 JWT Claims (`inventiv-api/src/auth.rs`)

**État actuel** :
```rust
struct Claims {
    iss: String,
    sub: String,                                // ✅ user_id
    email: String,                              // ✅ Présent
    role: String,                               // ✅ Présent (user global role)
    current_organization_id: Option<String>,   // ✅ Présent
    // ❌ MANQUE: current_organization_role: Option<String>,
    iat: usize,
    exp: usize,
}
```

**Problème** : Le rôle org n'est **pas** dans le JWT, donc :
- Il faut une requête DB à chaque décodage JWT pour obtenir le rôle
- Le Frontend ne peut pas connaître le rôle sans appeler `/auth/me`

---

### 1.3 Endpoint `/auth/login` (`inventiv-api/src/auth_endpoints.rs`)

**État actuel** :
```rust
// Ligne 97: Requête DB pour récupérer user
SELECT id, email, role, first_name, last_name, current_organization_id
FROM users
WHERE (username = $1 OR email = $1)
  AND password_hash = crypt($2, password_hash)

// Ligne 119-124: Création AuthUser
let auth_user = auth::AuthUser {
    user_id: u.id,
    email: u.email.clone(),
    role: u.role.clone(),
    current_organization_id: u.current_organization_id,  // ✅ Récupéré depuis DB
    // ❌ MANQUE: Résolution du rôle org depuis organization_memberships
};

// Ligne 125: Signature JWT
let token = auth::sign_session_jwt(&auth_user)  // ✅ JWT créé avec current_organization_id
```

**Problème** : Lors du login, le rôle org n'est **pas résolu** et n'est **pas inclus** dans le JWT.

---

### 1.4 Endpoint `/auth/me` (`inventiv-api/src/auth_endpoints.rs`)

**État actuel** :
```rust
// Ligne 160-175: Requête DB
SELECT
  u.username,
  u.email,
  u.role,
  u.first_name,
  u.last_name,
  u.current_organization_id,
  o.name as current_organization_name,
  o.slug as current_organization_slug
FROM users u
LEFT JOIN organizations o ON o.id = u.current_organization_id
WHERE u.id = $1

// Ligne 40-51: MeResponse
pub struct MeResponse {
    pub user_id: uuid::Uuid,
    pub username: String,
    pub email: String,
    pub role: String,                           // ✅ User global role
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub current_organization_id: Option<uuid::Uuid>,  // ✅ Présent
    pub current_organization_name: Option<String>,    // ✅ Présent
    pub current_organization_slug: Option<String>,    // ✅ Présent
    // ❌ MANQUE: current_organization_role: Option<String>,
}
```

**Problème** : Le rôle org n'est **pas** dans `MeResponse`, donc le Frontend ne peut pas le connaître.

---

### 1.5 Endpoint `/organizations/current` (Switch workspace)

**État actuel** (`inventiv-api/src/organizations.rs`) :
- `set_current_organization()` met à jour `users.current_organization_id`
- Crée un nouveau JWT avec `current_organization_id` mis à jour
- ❌ **MANQUE** : Résolution du rôle org et inclusion dans le nouveau JWT

---

### 1.6 Backend DB (Backup)

**État actuel** :
- ✅ `users.current_organization_id` → stocké en DB
- ✅ `organization_memberships.role` → stocké en DB
- ✅ Helper `get_membership_role(db, org_id, user_id)` existe dans `organizations.rs`

**Problème** : Le rôle org n'est **pas résolu automatiquement** lors du login/switch org pour être inclus dans le JWT.

---

## 2. État Actuel - Frontend (TypeScript)

### 2.1 Type `Me` (`inventiv-frontend/src/lib/types.ts`)

**État actuel** (à vérifier dans le fichier) :
```typescript
export type Me = {
  user_id: string;
  username: string;
  email: string;
  role: string;                                 // ✅ User global role
  first_name?: string | null;
  last_name?: string | null;
  current_organization_id?: string | null;     // ✅ Présent
  current_organization_name?: string | null;   // ✅ Présent
  current_organization_slug?: string | null;   // ✅ Présent
  // ❌ MANQUE: current_organization_role?: string | null;
};
```

**Problème** : Le Frontend ne peut pas connaître le rôle org sans appeler un endpoint supplémentaire.

---

## 3. Ce qui Manque (Checklist)

### Backend
- [ ] ❌ Ajouter `current_organization_role: Option<String>` dans `AuthUser` struct
- [ ] ❌ Ajouter `current_organization_role: Option<String>` dans JWT `Claims`
- [ ] ❌ Modifier `sign_session_jwt()` pour inclure le rôle org
- [ ] ❌ Modifier `decode_session_jwt()` pour extraire le rôle org
- [ ] ❌ Modifier `/auth/login` pour résoudre le rôle org depuis DB et l'inclure dans le JWT
- [ ] ❌ Modifier `/organizations/current` (switch org) pour résoudre le rôle org et mettre à jour le JWT
- [ ] ❌ Modifier `/auth/me` pour inclure `current_organization_role` dans `MeResponse`
- [ ] ❌ Créer helper `resolve_user_org_role(db, org_id, user_id)` si nécessaire (existe déjà : `get_membership_role`)

### Frontend
- [ ] ❌ Ajouter `current_organization_role?: string | null` dans type `Me`
- [ ] ❌ Utiliser le rôle org pour l'affichage conditionnel (Sidebar, pages, etc.)

---

## 4. Sécurité et Performance

### Sécurité Actuelle ✅
- ✅ JWT signé avec secret (`JWT_SECRET`)
- ✅ Cookie HttpOnly + SameSite=Lax
- ✅ TTL configurable (défaut: 12h)
- ✅ Validation issuer dans `decode_session_jwt()`

### Performance Actuelle ⚠️
- ⚠️ Le rôle org doit être résolu depuis DB à chaque requête (si nécessaire)
- ⚠️ Le Frontend doit appeler `/auth/me` pour obtenir le rôle org
- ✅ **Solution** : Inclure le rôle org dans le JWT → évite requête DB supplémentaire

### Backup DB ✅
- ✅ `users.current_organization_id` → source de vérité
- ✅ `organization_memberships.role` → source de vérité
- ✅ Si JWT corrompu/perdu → peut être régénéré depuis DB

---

## 5. Plan de Correction

### Phase 1 : Enrichir AuthUser et JWT avec Rôle Org

**Fichier** : `inventiv-api/src/auth.rs`

**Changements** :
1. Ajouter `current_organization_role: Option<String>` dans `AuthUser`
2. Ajouter `current_organization_role: Option<String>` dans `Claims`
3. Modifier `sign_session_jwt()` pour inclure le rôle org
4. Modifier `decode_session_jwt()` pour extraire le rôle org

### Phase 2 : Résoudre Rôle Org lors Login/Switch Org

**Fichier** : `inventiv-api/src/auth_endpoints.rs`

**Changements** :
1. Modifier `login()` pour résoudre le rôle org si `current_organization_id` existe
2. Inclure le rôle org dans le JWT lors du login

**Fichier** : `inventiv-api/src/organizations.rs`

**Changements** :
1. Modifier `set_current_organization()` pour résoudre le rôle org
2. Inclure le rôle org dans le nouveau JWT lors du switch

### Phase 3 : Enrichir MeResponse avec Rôle Org

**Fichier** : `inventiv-api/src/auth_endpoints.rs`

**Changements** :
1. Modifier la requête SQL dans `me()` pour joindre `organization_memberships` et récupérer le rôle
2. Ajouter `current_organization_role` dans `MeResponse`
3. Retourner le rôle org dans la réponse

### Phase 4 : Mettre à Jour Frontend

**Fichier** : `inventiv-frontend/src/lib/types.ts`

**Changements** :
1. Ajouter `current_organization_role?: string | null` dans type `Me`

---

## 6. Exemple de Code à Ajouter

### Helper pour résoudre le rôle org

```rust
// Dans organizations.rs (existe déjà)
async fn get_membership_role(
    db: &Pool<Postgres>,
    org_id: uuid::Uuid,
    user_id: uuid::Uuid,
) -> Option<rbac::OrgRole> {
    // ... code existant ...
}
```

### Modifier login() pour inclure rôle org

```rust
// Dans auth_endpoints.rs, fonction login()
let auth_user = auth::AuthUser {
    user_id: u.id,
    email: u.email.clone(),
    role: u.role.clone(),
    current_organization_id: u.current_organization_id,
    // AJOUTER :
    current_organization_role: if let Some(org_id) = u.current_organization_id {
        organizations::get_membership_role(&state.db, org_id, u.id)
            .await
            .map(|r| r.as_str().to_string())
    } else {
        None
    },
};
```

---

## 7. Résumé

### ✅ Ce qui est en place
- User info (user_id, email, role global) → ✅ Dans JWT + DB
- Organisation courante (id, name, slug) → ✅ Dans JWT + DB
- Sécurité JWT → ✅ Signé, HttpOnly, SameSite
- Backup DB → ✅ `users.current_organization_id` + `organization_memberships.role`

### ❌ Ce qui manque
- **Rôle org dans AuthUser** → ❌ Absent
- **Rôle org dans JWT** → ❌ Absent
- **Résolution automatique lors login/switch** → ❌ Non implémenté
- **Rôle org dans MeResponse** → ❌ Absent
- **Rôle org dans type TypeScript Me** → ❌ Absent

### Impact
- ⚠️ Performance : Requête DB supplémentaire pour obtenir le rôle org
- ⚠️ Frontend : Ne peut pas connaître le rôle org sans appel API supplémentaire
- ⚠️ RBAC : Impossible d'utiliser le rôle org dans les middlewares sans requête DB

---

## 8. Recommandation

**Action immédiate** : Implémenter les phases 1-3 pour enrichir la session avec le rôle org.

**Ordre d'implémentation** :
1. Phase 1 : Enrichir AuthUser et JWT
2. Phase 2 : Résoudre rôle lors login/switch
3. Phase 3 : Enrichir MeResponse
4. Phase 4 : Mettre à jour Frontend

**Bénéfices** :
- ✅ Performance : Pas de requête DB supplémentaire pour obtenir le rôle
- ✅ Sécurité : Rôle org dans JWT signé (non modifiable côté client)
- ✅ Frontend : Accès immédiat au rôle org depuis le JWT décodé
- ✅ RBAC : Middleware peut vérifier le rôle org sans requête DB

