# Proposition : Architecture de Sessions Multi-Organisation

## 🎯 Objectifs

1. **Séparer la notion de "current_organization" de la table `users`** → Elle appartient à la session, pas à l'utilisateur
2. **Permettre plusieurs sessions simultanées** avec des organisations différentes par session
3. **Persister les sessions en DB** avec organisation courante et rôle
4. **Synchroniser JWT ↔ DB** lors du switch d'organisation
5. **Sécurité renforcée** : invalidation, rotation, détection d'anomalies

---

## 🔍 Problème Actuel

### Architecture Actuelle (❌ Incorrecte)

```
users
├── id
├── email
├── current_organization_id  ← ❌ PROBLÈME : Un seul "current" par user
└── ...

JWT Claims
├── sub (user_id)
├── email
├── role
└── current_organization_id  ← ❌ Pas de session_id, pas de rôle org
```

**Problèmes** :
- ❌ Un utilisateur ne peut avoir qu'une seule organisation "courante" globale
- ❌ Impossible d'avoir plusieurs sessions avec des organisations différentes
- ❌ Pas de traçabilité des sessions actives
- ❌ Pas de moyen d'invalider une session spécifique
- ❌ Le rôle org n'est pas dans le JWT

---

## ✅ Architecture Proposée

### 1. Nouveau Modèle de Données

#### Table `user_sessions`

```sql
CREATE TABLE public.user_sessions (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    
    -- Session context
    current_organization_id uuid REFERENCES organizations(id) ON DELETE SET NULL,
    organization_role text CHECK (organization_role IN ('owner', 'admin', 'manager', 'user')),
    
    -- Security & tracking
    session_token_hash text NOT NULL,  -- Hash du JWT (pour invalidation)
    ip_address inet,
    user_agent text,
    
    -- Lifecycle
    created_at timestamptz NOT NULL DEFAULT now(),
    last_used_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    revoked_at timestamptz,  -- Soft delete pour audit
    
    -- Indexes
    CONSTRAINT user_sessions_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    CONSTRAINT user_sessions_org_id_fkey FOREIGN KEY (current_organization_id) REFERENCES organizations(id) ON DELETE SET NULL,
    CONSTRAINT user_sessions_org_role_check CHECK (organization_role IN ('owner', 'admin', 'manager', 'user'))
);

CREATE INDEX idx_user_sessions_user_id ON user_sessions(user_id) WHERE revoked_at IS NULL;
CREATE INDEX idx_user_sessions_token_hash ON user_sessions(session_token_hash) WHERE revoked_at IS NULL;
CREATE INDEX idx_user_sessions_expires_at ON user_sessions(expires_at) WHERE revoked_at IS NULL;
```

**Notes** :
- `session_token_hash` : Hash SHA256 du JWT complet (ou d'un `session_id` unique) pour invalidation
- `organization_role` : Rôle résolu depuis `organization_memberships` lors de la création/mise à jour
- `revoked_at` : Soft delete pour audit et sécurité

#### Retirer `current_organization_id` de `users`

```sql
-- Migration
ALTER TABLE users DROP COLUMN IF EXISTS current_organization_id;
```

---

### 2. JWT Claims Enrichis

```rust
struct Claims {
    iss: String,
    sub: String,                    // user_id
    email: String,
    role: String,                   // User global role (admin|user)
    
    // Session context
    session_id: String,              // ✅ NOUVEAU : UUID de la session en DB
    current_organization_id: Option<String>,
    current_organization_role: Option<String>,  // ✅ NOUVEAU : owner|admin|manager|user
    
    // Security
    iat: usize,
    exp: usize,
    jti: String,                    // ✅ NOUVEAU : JWT ID (pour rotation/invalidation)
}
```

**Bénéfices** :
- ✅ `session_id` permet de référencer la session en DB
- ✅ `current_organization_role` évite une requête DB supplémentaire
- ✅ `jti` permet l'invalidation/rotation de tokens

---

### 3. Flux de Connexion (Login)

```
1. User fournit credentials
2. Vérifier password_hash
3. Créer session en DB :
   - Générer session_id (UUID)
   - Résoudre current_organization_id (optionnel, depuis users.current_organization_id par défaut)
   - Résoudre organization_role depuis organization_memberships
   - Stocker ip_address, user_agent
   - Calculer expires_at (now + TTL)
4. Générer JWT avec :
   - session_id
   - current_organization_id
   - current_organization_role
   - jti (hash du session_id + secret)
5. Stocker session_token_hash dans DB (hash du JWT complet ou jti)
6. Retourner JWT dans cookie HttpOnly
```

**Code Rust** :
```rust
pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
    headers: HeaderMap,  // Pour ip_address, user_agent
) -> impl IntoResponse {
    // 1. Vérifier credentials
    let user = verify_credentials(&state.db, &req.email, &req.password).await?;
    
    // 2. Résoudre organisation par défaut (optionnel)
    let default_org_id = get_user_default_org(&state.db, user.id).await?;
    
    // 3. Résoudre rôle org si org_id existe
    let org_role = if let Some(org_id) = default_org_id {
        get_membership_role(&state.db, org_id, user.id).await?
    } else {
        None
    };
    
    // 4. Créer session en DB
    let session_id = uuid::Uuid::new_v4();
    let ip_address = extract_ip_address(&headers);
    let user_agent = extract_user_agent(&headers);
    let expires_at = chrono::Utc::now() + chrono::Duration::seconds(jwt_ttl_seconds() as i64);
    
    create_session(
        &state.db,
        session_id,
        user.id,
        default_org_id,
        org_role.clone(),
        ip_address,
        user_agent,
        expires_at,
    ).await?;
    
    // 5. Générer JWT
    let auth_user = AuthUser {
        user_id: user.id,
        email: user.email,
        role: user.role,
        session_id: session_id.to_string(),
        current_organization_id: default_org_id,
        current_organization_role: org_role,
    };
    let token = sign_session_jwt(&auth_user)?;
    
    // 6. Stocker hash du token en DB (pour invalidation)
    let token_hash = sha256(&token);
    update_session_token_hash(&state.db, session_id, &token_hash).await?;
    
    // 7. Retourner cookie
    let cookie = session_cookie_value(&token);
    Ok(Json(LoginResponse { ... }).with_header(SET_COOKIE, cookie))
}
```

---

### 4. Flux de Switch d'Organisation

```
1. User demande switch vers org_id
2. Vérifier que user est membre de org_id
3. Résoudre organization_role depuis organization_memberships
4. Mettre à jour session en DB :
   - UPDATE user_sessions SET current_organization_id = $1, organization_role = $2, last_used_at = NOW()
   - WHERE id = $session_id AND user_id = $user_id AND revoked_at IS NULL
5. Régénérer JWT avec nouvelles valeurs
6. Mettre à jour session_token_hash en DB
7. Retourner nouveau JWT dans cookie
```

**Code Rust** :
```rust
pub async fn set_current_organization(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(user): axum::extract::Extension<auth::AuthUser>,
    Json(req): Json<SetCurrentOrganizationRequest>,
) -> impl IntoResponse {
    let session_id = uuid::Uuid::parse_str(&user.session_id)?;
    
    // 1. Vérifier membership
    if let Some(org_id) = req.organization_id {
        if !is_member(&state.db, org_id, user.user_id).await? {
            return Err(StatusCode::FORBIDDEN);
        }
        let org_role = get_membership_role(&state.db, org_id, user.user_id).await?
            .ok_or(StatusCode::FORBIDDEN)?;
        
        // 2. Mettre à jour session en DB
        update_session_org(
            &state.db,
            session_id,
            Some(org_id),
            Some(org_role.as_str().to_string()),
        ).await?;
        
        // 3. Régénérer JWT
        let updated_user = AuthUser {
            current_organization_id: Some(org_id),
            current_organization_role: Some(org_role.as_str().to_string()),
            ..user
        };
        let new_token = sign_session_jwt(&updated_user)?;
        
        // 4. Mettre à jour token_hash
        update_session_token_hash(&state.db, session_id, &sha256(&new_token)).await?;
        
        Ok(Json(SetCurrentOrganizationResponse { ... })
            .with_header(SET_COOKIE, session_cookie_value(&new_token)))
    } else {
        // Switch vers Personal (pas d'org)
        update_session_org(&state.db, session_id, None, None).await?;
        let updated_user = AuthUser {
            current_organization_id: None,
            current_organization_role: None,
            ..user
        };
        let new_token = sign_session_jwt(&updated_user)?;
        update_session_token_hash(&state.db, session_id, &sha256(&new_token)).await?;
        Ok(Json(SetCurrentOrganizationResponse { ... })
            .with_header(SET_COOKIE, session_cookie_value(&new_token)))
    }
}
```

---

### 5. Validation de Session (Middleware)

```
1. Extraire JWT depuis cookie/Bearer
2. Décoder JWT → obtenir session_id
3. Vérifier session en DB :
   - SELECT * FROM user_sessions
   - WHERE id = $session_id
   - AND revoked_at IS NULL
   - AND expires_at > NOW()
   - AND session_token_hash = $token_hash (optionnel, pour sécurité renforcée)
4. Si session valide :
   - UPDATE user_sessions SET last_used_at = NOW() WHERE id = $session_id
   - Extraire AuthUser depuis JWT
5. Si session invalide :
   - Retourner 401 Unauthorized
```

**Code Rust** :
```rust
pub async fn require_user(
    State(db): State<Pool<Postgres>>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    let token = extract_cookie(req.headers(), &session_cookie_name())
        .or_else(|| extract_bearer(req.headers()))
        .ok_or_else(|| StatusCode::UNAUTHORIZED)?;
    
    // 1. Décoder JWT
    let claims = decode_session_jwt(&token)?;
    let session_id = uuid::Uuid::parse_str(&claims.session_id)?;
    
    // 2. Vérifier session en DB
    let session_valid = verify_session_db(&db, session_id, &token).await?;
    if !session_valid {
        return (StatusCode::UNAUTHORIZED, Json(json!({"error":"session_invalid"}))).into_response();
    }
    
    // 3. Mettre à jour last_used_at
    update_session_last_used(&db, session_id).await.ok();
    
    // 4. Extraire AuthUser
    let user = AuthUser {
        user_id: uuid::Uuid::parse_str(&claims.sub)?,
        email: claims.email,
        role: claims.role,
        session_id: claims.session_id,
        current_organization_id: claims.current_organization_id.map(|s| uuid::Uuid::parse_str(&s).ok()).flatten(),
        current_organization_role: claims.current_organization_role,
    };
    
    req.extensions_mut().insert(user);
    next.run(req).await
}
```

---

### 6. Logout

```
1. Extraire session_id depuis JWT
2. Marquer session comme révoquée :
   - UPDATE user_sessions SET revoked_at = NOW() WHERE id = $session_id
3. Retourner cookie vide (Max-Age=0)
```

**Code Rust** :
```rust
pub async fn logout(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(user): axum::extract::Extension<auth::AuthUser>,
) -> impl IntoResponse {
    let session_id = uuid::Uuid::parse_str(&user.session_id)?;
    revoke_session(&state.db, session_id).await.ok();
    
    let cookie = clear_session_cookie_value();
    Ok(Json(json!({"status":"ok"})).with_header(SET_COOKIE, cookie))
}
```

---

## 🔒 Améliorations de Sécurité

### 1. Invalidation de Session

**Endpoint** : `POST /auth/sessions/:session_id/revoke`

```rust
pub async fn revoke_session_endpoint(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(user): axum::extract::Extension<auth::AuthUser>,
    Path(session_id): Path<uuid::Uuid>,
) -> impl IntoResponse {
    // Vérifier que session_id appartient à user_id
    let session = get_session(&state.db, session_id).await?;
    if session.user_id != user.user_id {
        return Err(StatusCode::FORBIDDEN);
    }
    
    revoke_session(&state.db, session_id).await?;
    Ok(Json(json!({"status":"ok"})))
}
```

**Cas d'usage** :
- User veut déconnecter une session spécifique (ex: session sur un autre appareil)
- Admin veut révoquer toutes les sessions d'un user compromis

---

### 2. Liste des Sessions Actives

**Endpoint** : `GET /auth/sessions`

```rust
pub async fn list_sessions(
    State(state): State<Arc<AppState>>,
    axum::extract::Extension(user): axum::extract::Extension<auth::AuthUser>,
) -> impl IntoResponse {
    let sessions = get_user_sessions(&state.db, user.user_id).await?;
    Ok(Json(sessions))
}
```

**Réponse** :
```json
[
  {
    "session_id": "uuid",
    "current_organization_id": "uuid",
    "current_organization_name": "Org Name",
    "organization_role": "owner",
    "ip_address": "192.168.1.1",
    "user_agent": "Mozilla/5.0...",
    "created_at": "2025-01-06T10:00:00Z",
    "last_used_at": "2025-01-06T12:00:00Z",
    "expires_at": "2025-01-06T22:00:00Z",
    "is_current": true  // true si session_id == session courante
  }
]
```

---

### 3. Rotation de Tokens

**Stratégie** : Régénérer le JWT périodiquement (ex: toutes les heures) pour limiter la fenêtre d'exposition en cas de vol.

```rust
pub async fn require_user_with_rotation(
    State(db): State<Pool<Postgres>>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    // ... validation session ...
    
    // Si last_used_at > 1h, régénérer token
    if session.last_used_at < chrono::Utc::now() - chrono::Duration::hours(1) {
        let new_token = rotate_session_token(&db, session_id).await?;
        // Retourner nouveau token dans header X-New-Session-Token
        // Frontend doit mettre à jour le cookie
    }
    
    // ... continuer ...
}
```

---

### 4. Détection d'Anomalies

**Indicateurs** :
- Changement d'IP brutale
- Changement de user_agent
- Plusieurs sessions depuis des IPs géographiquement distantes
- Tentative d'accès avec un token révoqué

**Logging** :
```rust
// Dans require_user middleware
if session.ip_address != current_ip {
    log::warn!("IP change detected for session {}", session_id);
    // Optionnel : demander re-authentification
}

if session.revoked_at.is_some() {
    log::warn!("Revoked session access attempt: {}", session_id);
    return Err(StatusCode::UNAUTHORIZED);
}
```

---

### 5. Expiration et Nettoyage

**Job de nettoyage** (cron) :
```rust
pub async fn cleanup_expired_sessions(db: &Pool<Postgres>) {
    sqlx::query(
        "UPDATE user_sessions SET revoked_at = NOW() WHERE expires_at < NOW() AND revoked_at IS NULL"
    )
    .execute(db)
    .await
    .ok();
}
```

**Exécution** : Toutes les heures via `tokio::spawn` ou job scheduler.

---

## 📊 Migration Plan

### Phase 1 : Créer Table `user_sessions`

```sql
-- Migration: 20250106000000_create_user_sessions.sql
CREATE TABLE public.user_sessions (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    current_organization_id uuid REFERENCES organizations(id) ON DELETE SET NULL,
    organization_role text CHECK (organization_role IN ('owner', 'admin', 'manager', 'user')),
    session_token_hash text NOT NULL,
    ip_address inet,
    user_agent text,
    created_at timestamptz NOT NULL DEFAULT now(),
    last_used_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL,
    revoked_at timestamptz,
    CONSTRAINT user_sessions_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    CONSTRAINT user_sessions_org_id_fkey FOREIGN KEY (current_organization_id) REFERENCES organizations(id) ON DELETE SET NULL
);

CREATE INDEX idx_user_sessions_user_id ON user_sessions(user_id) WHERE revoked_at IS NULL;
CREATE INDEX idx_user_sessions_token_hash ON user_sessions(session_token_hash) WHERE revoked_at IS NULL;
CREATE INDEX idx_user_sessions_expires_at ON user_sessions(expires_at) WHERE revoked_at IS NULL;
```

### Phase 2 : Migrer Sessions Existantes

```sql
-- Migration: 20250106000001_migrate_existing_sessions.sql
-- Pour chaque user avec current_organization_id, créer une session "legacy"
INSERT INTO user_sessions (
    user_id,
    current_organization_id,
    organization_role,
    session_token_hash,
    created_at,
    last_used_at,
    expires_at
)
SELECT
    u.id,
    u.current_organization_id,
    om.role,
    encode(digest(gen_random_uuid()::text, 'sha256'), 'hex'),  -- Placeholder
    NOW(),
    NOW(),
    NOW() + INTERVAL '12 hours'
FROM users u
LEFT JOIN organization_memberships om ON om.organization_id = u.current_organization_id AND om.user_id = u.id
WHERE u.current_organization_id IS NOT NULL;
```

### Phase 3 : Retirer `current_organization_id` de `users`

```sql
-- Migration: 20250106000002_remove_current_org_from_users.sql
ALTER TABLE users DROP COLUMN IF EXISTS current_organization_id;
```

### Phase 4 : Mettre à Jour Code Rust

1. Enrichir `AuthUser` avec `session_id`, `current_organization_role`
2. Enrichir JWT `Claims` avec `session_id`, `current_organization_role`, `jti`
3. Modifier `login()` pour créer session en DB
4. Modifier `set_current_organization()` pour mettre à jour session en DB
5. Modifier `require_user()` pour valider session en DB
6. Modifier `logout()` pour révoquer session en DB
7. Ajouter endpoints `/auth/sessions` et `/auth/sessions/:id/revoke`

### Phase 5 : Mettre à Jour Frontend

1. Ajouter `session_id`, `current_organization_role` dans type `Me`
2. Gérer rotation de tokens (header `X-New-Session-Token`)
3. Ajouter UI pour lister/révoquer sessions actives

---

## ✅ Avantages de cette Architecture

1. **Multi-session** : Un user peut avoir plusieurs sessions avec des orgs différentes
2. **Sécurité** : Invalidation granulaire, traçabilité, détection d'anomalies
3. **Performance** : Rôle org dans JWT → pas de requête DB supplémentaire
4. **Audit** : Historique complet des sessions (soft delete)
5. **Flexibilité** : Rotation de tokens, expiration configurable
6. **Séparation des responsabilités** : `users` = identité, `user_sessions` = contexte de session

---

## ❓ Questions / Remarques

### 1. Hash du Token vs Session ID

**Option A** : Stocker `session_token_hash` (hash du JWT complet)
- ✅ Sécurité maximale : invalidation immédiate si token volé
- ❌ Nécessite recalculer hash à chaque validation

**Option B** : Stocker uniquement `session_id` dans JWT, pas de hash
- ✅ Plus simple, moins de requêtes DB
- ❌ Si token volé, reste valide jusqu'à expiration

**Recommandation** : **Option B** pour MVP, puis **Option A** si besoin de sécurité renforcée.

---

### 2. Organisation par Défaut au Login

**Question** : Lors du login, quelle organisation doit être sélectionnée par défaut ?

**Options** :
- **A** : Dernière organisation utilisée (nécessite historique)
- **B** : Première organisation par ordre alphabétique
- **C** : Organisation avec rôle le plus élevé (owner > admin > manager > user)
- **D** : Aucune organisation (Personal mode)

**Recommandation** : **Option C** (rôle le plus élevé) ou **Option D** (Personal) pour MVP.

---

### 3. Limite de Sessions Actives

**Question** : Faut-il limiter le nombre de sessions actives par user ?

**Options** :
- **A** : Pas de limite
- **B** : Limite fixe (ex: 10 sessions)
- **C** : Limite configurable par user/org

**Recommandation** : **Option B** (limite fixe de 10) pour MVP, avec message d'erreur clair.

---

### 4. Synchronisation JWT ↔ DB

**Question** : Que faire si le JWT et la DB sont désynchronisés ?

**Scénario** : User switch org dans un onglet, puis utilise un autre onglet avec ancien JWT.

**Options** :
- **A** : Rejeter la requête (401) et forcer re-login
- **B** : Accepter le JWT mais mettre à jour la session en DB (dernier write wins)
- **C** : Comparer `last_used_at` et utiliser la session la plus récente

**Recommandation** : **Option A** pour sécurité, avec message clair "Votre session a été mise à jour, veuillez vous reconnecter".

---

## 🚀 Plan d'Implémentation

### Étape 1 : Migration DB
- [ ] Créer table `user_sessions`
- [ ] Migrer sessions existantes (si applicable)
- [ ] Retirer `current_organization_id` de `users`

### Étape 2 : Backend Rust
- [ ] Enrichir `AuthUser` avec `session_id`, `current_organization_role`
- [ ] Enrichir JWT `Claims` avec `session_id`, `current_organization_role`, `jti`
- [ ] Modifier `login()` pour créer session en DB
- [ ] Modifier `set_current_organization()` pour mettre à jour session en DB
- [ ] Modifier `require_user()` pour valider session en DB
- [ ] Modifier `logout()` pour révoquer session en DB
- [ ] Ajouter endpoints `/auth/sessions` et `/auth/sessions/:id/revoke`

### Étape 3 : Frontend
- [ ] Ajouter `session_id`, `current_organization_role` dans type `Me`
- [ ] Gérer rotation de tokens (si implémentée)
- [ ] Ajouter UI pour lister/révoquer sessions actives

### Étape 4 : Tests
- [ ] Tests unitaires pour création/validation/révocation de sessions
- [ ] Tests d'intégration pour login/logout/switch org
- [ ] Tests de sécurité (invalidation, expiration, multi-session)

---

## 📝 Conclusion

Cette architecture permet :
- ✅ **Multi-session** avec organisations différentes
- ✅ **Sécurité renforcée** (invalidation, audit, détection)
- ✅ **Performance** (rôle org dans JWT)
- ✅ **Flexibilité** (rotation, expiration configurable)

**Prochaine étape** : Valider cette proposition et commencer l'implémentation par la migration DB.

