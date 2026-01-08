# État d'Implémentation : Gestion des Sessions Multi-Organisation

**Date** : 2025-01-06  
**Contexte** : Analyse de ce qui est fait vs ce qui reste à faire pour l'architecture de sessions multi-org

---

## ✅ Ce qui est DÉJÀ IMPLÉMENTÉ

### 1. Base de Données ✅

#### Table `user_sessions` créée
- ✅ Migration `20260107000000_create_user_sessions.sql` existe
- ✅ Table dans `00000000000000_baseline.sql` (ligne 643)
- ✅ Colonnes : `id`, `user_id`, `current_organization_id`, `organization_role`, `session_token_hash`, `ip_address`, `user_agent`, `created_at`, `last_used_at`, `expires_at`, `revoked_at`
- ✅ Contraintes : FOREIGN KEY vers `users` et `organizations`, CHECK sur `organization_role`
- ✅ Index : `user_id`, `token_hash`, `expires_at`, `org_id` (avec filtre `revoked_at IS NULL`)

#### Migration des sessions existantes
- ✅ Migration `20260107000001_migrate_existing_sessions.sql` existe
- ✅ Migre `users.current_organization_id` vers `user_sessions` pour les users existants

#### Retrait de `current_organization_id` de `users`
- ✅ Migration `20260107000002_remove_current_org_from_users.sql` existe
- ⚠️ **À vérifier** : La colonne est-elle toujours dans `baseline.sql` ou a-t-elle été retirée ?

---

### 2. Backend Rust - Module Auth ✅

#### Struct `AuthUser` enrichi (`inventiv-api/src/auth.rs`)
```rust
pub struct AuthUser {
    pub user_id: uuid::Uuid,
    pub email: String,
    pub role: String,
    pub session_id: String,                      // ✅ AJOUTÉ
    pub current_organization_id: Option<uuid::Uuid>,
    pub current_organization_role: Option<String>,  // ✅ AJOUTÉ
}
```

#### JWT Claims enrichis (`inventiv-api/src/auth.rs`)
```rust
struct Claims {
    iss: String,
    sub: String,
    email: String,
    role: String,
    session_id: String,                         // ✅ AJOUTÉ
    current_organization_id: Option<String>,
    current_organization_role: Option<String>,   // ✅ AJOUTÉ
    jti: String,                                 // ✅ AJOUTÉ
    iat: usize,
    exp: usize,
}
```

#### Fonctions helpers implémentées (`inventiv-api/src/auth.rs`)
- ✅ `create_session()` - Créer une session en DB
- ✅ `verify_session_db()` - Vérifier qu'une session est valide (non révoquée, non expirée)
- ✅ `update_session_last_used()` - Mettre à jour `last_used_at`
- ✅ `update_session_org()` - Mettre à jour `current_organization_id` et `organization_role`
- ✅ `update_session_token_hash()` - Mettre à jour le hash du token (rotation)
- ✅ `revoke_session()` - Révoquer une session (soft delete)
- ✅ `get_user_last_org()` - Récupérer la dernière org utilisée par un user
- ✅ `extract_ip_address()` - Extraire IP depuis headers
- ✅ `extract_user_agent()` - Extraire User-Agent depuis headers
- ✅ `hash_session_token()` - Hasher un token JWT (SHA256)

#### Middleware `require_user()` modifié
- ✅ Décodage JWT avec `session_id` et `current_organization_role`
- ✅ Vérification session en DB via `verify_session_db()`
- ✅ Mise à jour `last_used_at` automatique
- ✅ Gestion erreurs (session invalide/expirée/révoquée)

#### Middleware `require_user_or_api_key()` modifié
- ✅ Vérification session en DB pour les sessions cookie/Bearer
- ✅ Mise à jour `last_used_at` automatique

---

### 3. Backend Rust - Endpoints Auth ✅

#### `POST /auth/login` (`inventiv-api/src/auth_endpoints.rs`)
- ✅ Récupère dernière org utilisée via `get_user_last_org()`
- ✅ Résout `organization_role` depuis `organization_memberships`
- ✅ Crée session en DB avec `create_session()`
- ✅ Génère JWT avec `session_id` + `current_organization_role`
- ✅ Stocke hash du token en DB
- ✅ Retourne JWT dans cookie HttpOnly

#### `POST /auth/logout` (`inventiv-api/src/auth_endpoints.rs`)
- ✅ Révoque session en DB via `revoke_session()`
- ✅ Retourne cookie vide

#### `GET /auth/me` (`inventiv-api/src/auth_endpoints.rs`)
- ✅ Récupère `current_organization_id` depuis JWT (via `AuthUser`)
- ✅ Récupère `current_organization_name` et `current_organization_slug` depuis DB
- ⚠️ **MANQUE** : `current_organization_role` dans `MeResponse` (mais disponible dans `AuthUser` depuis JWT)

---

## ❌ Ce qui MANQUE / À COMPLÉTER

### 1. Backend Rust - Endpoint Switch Organisation ✅

#### `PUT /organizations/current` (`inventiv-api/src/organizations.rs`)
**État actuel** : ✅ **DÉJÀ IMPLÉMENTÉ**

**Ce qui est fait** :
- ✅ `set_current_organization()` utilise `update_session_org()` pour mettre à jour la session en DB
- ✅ Résout `organization_role` depuis `organization_memberships`
- ✅ Régénère JWT avec nouvelles valeurs (`current_organization_id` + `organization_role`)
- ✅ Met à jour `session_token_hash` en DB avec nouveau token
- ✅ Retourne nouveau JWT dans cookie
- ✅ Gère le cas "Personal mode" (org_id = None)

**Code attendu** :
```rust
pub async fn set_current_organization(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(user): axum::extract::Extension<auth::AuthUser>,
    Json(req): Json<SetCurrentOrganizationRequest>,
) -> impl IntoResponse {
    let session_id = uuid::Uuid::parse_str(&user.session_id)?;
    
    if let Some(org_id) = req.organization_id {
        // Vérifier membership
        if !is_member(&state.db, org_id, user.user_id).await? {
            return Err(StatusCode::FORBIDDEN);
        }
        
        // Résoudre rôle org
        let org_role = get_membership_role(&state.db, org_id, user.user_id).await?
            .ok_or(StatusCode::FORBIDDEN)?;
        
        // Mettre à jour session en DB
        auth::update_session_org(
            &state.db,
            session_id,
            Some(org_id),
            Some(org_role.as_str().to_string()),
        ).await?;
        
        // Régénérer JWT
        let updated_user = auth::AuthUser {
            current_organization_id: Some(org_id),
            current_organization_role: Some(org_role.as_str().to_string()),
            ..user
        };
        let new_token = auth::sign_session_jwt(&updated_user)?;
        
        // Mettre à jour token_hash
        auth::update_session_token_hash(&state.db, session_id, &auth::hash_session_token(&new_token)).await?;
        
        Ok(Json(SetCurrentOrganizationResponse { ... })
            .with_header(SET_COOKIE, auth::session_cookie_value(&new_token)))
    } else {
        // Switch vers Personal (pas d'org)
        auth::update_session_org(&state.db, session_id, None, None).await?;
        let updated_user = auth::AuthUser {
            current_organization_id: None,
            current_organization_role: None,
            ..user
        };
        let new_token = auth::sign_session_jwt(&updated_user)?;
        auth::update_session_token_hash(&state.db, session_id, &auth::hash_session_token(&new_token)).await?;
        Ok(Json(SetCurrentOrganizationResponse { ... })
            .with_header(SET_COOKIE, auth::session_cookie_value(&new_token)))
    }
}
```

---

### 2. Backend Rust - Endpoints Sessions Manquants ⏳

#### `GET /auth/sessions` - Liste des sessions actives
**À créer** :
- [ ] Endpoint pour lister toutes les sessions actives d'un user
- [ ] Retourner : `session_id`, `current_organization_id`, `current_organization_name`, `organization_role`, `ip_address`, `user_agent`, `created_at`, `last_used_at`, `expires_at`, `is_current` (bool)

**Code attendu** :
```rust
#[derive(Debug, Serialize)]
pub struct SessionResponse {
    pub session_id: uuid::Uuid,
    pub current_organization_id: Option<uuid::Uuid>,
    pub current_organization_name: Option<String>,
    pub organization_role: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_used_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub is_current: bool,  // true si session_id == session courante
}

pub async fn list_sessions(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(user): axum::extract::Extension<auth::AuthUser>,
) -> impl IntoResponse {
    let current_session_id = uuid::Uuid::parse_str(&user.session_id).ok();
    
    let rows: Vec<(uuid::Uuid, Option<uuid::Uuid>, Option<String>, Option<String>, Option<String>, Option<String>, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        r#"
        SELECT 
            us.id,
            us.current_organization_id,
            o.name as current_organization_name,
            us.organization_role,
            us.ip_address::text,
            us.user_agent,
            us.created_at,
            us.last_used_at,
            us.expires_at
        FROM user_sessions us
        LEFT JOIN organizations o ON o.id = us.current_organization_id
        WHERE us.user_id = $1
          AND us.revoked_at IS NULL
          AND us.expires_at > NOW()
        ORDER BY us.last_used_at DESC
        "#,
    )
    .bind(user.user_id)
    .fetch_all(&state.db)
    .await
    .ok()
    .unwrap_or_default();
    
    let sessions: Vec<SessionResponse> = rows.into_iter().map(|(id, org_id, org_name, role, ip, ua, created, last_used, expires)| {
        SessionResponse {
            session_id: id,
            current_organization_id: org_id,
            current_organization_name: org_name,
            organization_role: role,
            ip_address: ip,
            user_agent: ua,
            created_at: created,
            last_used_at: last_used,
            expires_at: expires,
            is_current: Some(id) == current_session_id,
        }
    }).collect();
    
    Ok(Json(sessions))
}
```

#### `POST /auth/sessions/:session_id/revoke` - Révoquer une session spécifique
**À créer** :
- [ ] Endpoint pour révoquer une session spécifique
- [ ] Vérifier que `session_id` appartient à `user_id` (sécurité)
- [ ] Appeler `revoke_session()`

**Code attendu** :
```rust
pub async fn revoke_session_endpoint(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(user): axum::extract::Extension<auth::AuthUser>,
    Path(session_id): Path<uuid::Uuid>,
) -> impl IntoResponse {
    // Vérifier que session_id appartient à user_id
    let session_user_id: Option<uuid::Uuid> = sqlx::query_scalar(
        "SELECT user_id FROM user_sessions WHERE id = $1 AND revoked_at IS NULL"
    )
    .bind(session_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();
    
    if session_user_id != Some(user.user_id) {
        return Err(StatusCode::FORBIDDEN);
    }
    
    auth::revoke_session(&state.db, session_id).await?;
    Ok(Json(json!({"status":"ok"})))
}
```

---

### 3. Backend Rust - Enrichir MeResponse ⏳

#### `GET /auth/me` - Ajouter `current_organization_role`
**À faire** :
- [ ] Ajouter `current_organization_role: Option<String>` dans `MeResponse`
- [ ] Récupérer depuis JWT (via `AuthUser`) au lieu de faire une requête DB supplémentaire

**Code attendu** :
```rust
pub struct MeResponse {
    pub user_id: uuid::Uuid,
    pub username: String,
    pub email: String,
    pub role: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub current_organization_id: Option<uuid::Uuid>,
    pub current_organization_name: Option<String>,
    pub current_organization_slug: Option<String>,
    pub current_organization_role: Option<String>,  // ✅ AJOUTER
}

// Dans la fonction me() :
Json(MeResponse {
    // ...
    current_organization_role: user.current_organization_role,  // Depuis JWT
})
```

---

### 4. Frontend TypeScript ⏳

#### Type `Me` - Ajouter `current_organization_role`
**Fichier** : `inventiv-frontend/src/components/account/AccountSection.tsx`

**À faire** :
- [ ] Ajouter `current_organization_role?: string | null` dans type `Me`

**Code attendu** :
```typescript
export type Me = WorkspaceMe & {
  user_id: string;
  username: string;
  email: string;
  role: string;
  first_name?: string | null;
  last_name?: string | null;
  current_organization_name?: string | null;
  current_organization_slug?: string | null;
  current_organization_role?: string | null;  // ✅ AJOUTER
};
```

#### UI - Liste des Sessions Actives
**À créer** :
- [ ] Nouvelle page ou section dans `AccountSection` pour lister sessions actives
- [ ] Afficher : IP, User-Agent, Organisation, Rôle, Créée le, Dernière utilisation, Expire le
- [ ] Badge "Session courante" sur la session active
- [ ] Bouton "Révoquer" pour chaque session (sauf la courante)
- [ ] Confirmation avant révocation

**Fichiers** :
- `inventiv-frontend/src/components/account/SessionsDialog.tsx` (nouveau)
- `inventiv-frontend/src/components/account/AccountSection.tsx` (ajouter bouton)

---

### 5. Base de Données - Vérifications ⏳

#### Vérifier que `current_organization_id` a été retiré de `users`
**À faire** :
- [ ] Vérifier dans `00000000000000_baseline.sql` si la colonne existe encore
- [ ] Si oui, créer une migration pour la retirer (ou mettre à jour baseline)

#### Vérifier que les migrations ont été appliquées
**À faire** :
- [ ] Vérifier que `20260107000000_create_user_sessions.sql` a été appliquée
- [ ] Vérifier que `20260107000001_migrate_existing_sessions.sql` a été appliquée
- [ ] Vérifier que `20260107000002_remove_current_org_from_users.sql` a été appliquée

---

### 6. Tests ⏳

#### Tests Unitaires
- [ ] Tests pour `create_session()`, `verify_session_db()`, `update_session_org()`, `revoke_session()`
- [ ] Tests pour `get_user_last_org()`

#### Tests d'Intégration
- [ ] Test login → vérifier session créée en DB
- [ ] Test logout → vérifier session révoquée
- [ ] Test switch org → vérifier session mise à jour + nouveau JWT
- [ ] Test multi-sessions → créer 2 sessions avec orgs différentes
- [ ] Test révocation session → vérifier que session révoquée ne fonctionne plus

#### Tests Manuels
- [ ] Login → vérifier cookie JWT contient `session_id` + `current_organization_role`
- [ ] Switch org → vérifier nouveau cookie avec nouvelle org + rôle
- [ ] Liste sessions → vérifier affichage correct
- [ ] Révoquer session → vérifier que session ne fonctionne plus

---

## 📋 Checklist Complète

### Backend Rust
- [x] Table `user_sessions` créée
- [x] `AuthUser` enrichi avec `session_id` + `current_organization_role`
- [x] JWT Claims enrichis avec `session_id` + `current_organization_role` + `jti`
- [x] Fonctions helpers implémentées
- [x] `login()` modifié pour créer session en DB
- [x] `logout()` modifié pour révoquer session
- [x] `require_user()` modifié pour vérifier session en DB
- [x] `set_current_organization()` modifié pour mettre à jour session en DB ✅
- [ ] `GET /auth/sessions` créé (liste sessions actives)
- [ ] `POST /auth/sessions/:id/revoke` créé (révoquer session)
- [ ] `MeResponse` enrichi avec `current_organization_role`

### Frontend TypeScript
- [ ] Type `Me` enrichi avec `current_organization_role`
- [ ] UI liste sessions actives créée
- [ ] UI révocation session créée

### Base de Données
- [x] Migration `create_user_sessions` créée
- [x] Migration `migrate_existing_sessions` créée
- [x] Migration `remove_current_org_from_users` créée
- [ ] Vérifier que `current_organization_id` a été retiré de `users` dans baseline
- [ ] Vérifier que les migrations ont été appliquées

### Tests
- [ ] Tests unitaires créés
- [ ] Tests d'intégration créés
- [ ] Tests manuels effectués

---

## 🎯 Plan d'Action Recommandé

### Étape 1 : Vérifications DB (15 min)
1. Vérifier état de `current_organization_id` dans `users` (baseline.sql)
2. Vérifier que les migrations ont été appliquées

### Étape 2 : Compléter Backend (1-2h)
1. Modifier `set_current_organization()` pour utiliser `update_session_org()`
2. Créer endpoint `GET /auth/sessions`
3. Créer endpoint `POST /auth/sessions/:id/revoke`
4. Enrichir `MeResponse` avec `current_organization_role`

### Étape 3 : Compléter Frontend (1-2h)
1. Ajouter `current_organization_role` dans type `Me`
2. Créer composant `SessionsDialog.tsx`
3. Intégrer dans `AccountSection.tsx`

### Étape 4 : Tests (1h)
1. Tests unitaires
2. Tests d'intégration
3. Tests manuels

---

## 📊 État Global

**Progression** : ~85% complété

- ✅ **Fondations** : Table DB, structs Rust, helpers, login/logout, switch org
- ⏳ **À compléter** : Endpoints sessions (liste/révocation), Frontend, `MeResponse` enrichi, tests

**Estimation temps restant** : 2-3h de développement + 1h de tests

---

## 🔍 Points d'Attention

1. **Compatibilité backward** : Les sessions legacy (créées avant migration) doivent être gérées
2. **Performance** : Vérification session en DB à chaque requête → acceptable si index optimisés
3. **Sécurité** : Vérifier que `session_id` dans JWT ne peut pas être falsifié (signature JWT)
4. **Expiration** : Job de nettoyage automatique des sessions expirées (optionnel)

---

**Prochaine étape** : Commencer par vérifier l'état de `set_current_organization()` et compléter les endpoints manquants.

