# État des lieux CI/CD - Inventiv Agents

**Date**: 2025-01-08  
**Branche**: `main`  
**Dernier commit**: `67944e8` - docs: consolidate and reorganize documentation structure

---

## 📊 État Git

### Fichiers modifiés non commités
- `docs/domain_design_and_data_model.md` (modifié)
- `inventiv-api/src/organizations.rs` (modifié)
- `inventiv-api/src/rbac.rs` (modifié)
- `inventiv-common/src/lib.rs` (modifié)
- `inventiv-orchestrator/src/migrations.rs` (modifié)
- `inventiv-orchestrator/src/state_machine.rs` (modifié)
- `sqlx-migrations/00000000000000_baseline.sql` (modifié)
- `sqlx-migrations/20260108000001_add_user_account_plan_and_wallet.sql` (nouveau)
- `sqlx-migrations/20260108000002_add_org_subscription_plan_and_wallet.sql` (nouveau)
- `sqlx-migrations/20260108000003_add_instances_organization_id.sql` (nouveau)
- `sqlx-migrations/20260108000004_add_instances_double_activation.sql` (nouveau)

### Branches actives
- `main` (branche courante)
- `feat/finops-dashboard`
- `feat/finops-eur-dashboard`
- `full-i18n`
- `wip/batch-20251218-1406`
- `worker-fixes`

---

## 🏗️ Architecture CI/CD

### Workflows GitHub Actions

#### 1. **CI** (`.github/workflows/ci.yml`)
- ✅ **Status**: Opérationnel
- **Déclenchement**: PR + push sur `main`
- **Jobs**:
  - `rust`: fmt-check, clippy, test, security-check, agent-version-check
  - `frontend`: npm ci, lint, build (avec vérification lightningcss)
- **Réutilisable**: Oui (`workflow_call`)

#### 2. **Deploy Staging** (`.github/workflows/deploy-staging.yml`)
- ❌ **Status**: **PROBLÈME IDENTIFIÉ** - Auto-deploy ne fonctionne pas
- **Déclenchement**: Push sur `main` + `workflow_dispatch`
- **Pipeline**:
  1. CI gate (réutilise `ci.yml`)
  2. Build ARM64 (QEMU + buildx)
  3. Push GHCR (tag SHA12)
  4. Promote SHA → `:staging`
  5. Deploy (`make stg-update`)

**Problèmes identifiés**:
- ❌ **Heredoc avec single quotes** (`'EOF'`) dans "Write staging env file" → Les variables GitHub Actions ne sont **pas substituées**
- ❌ **Syntaxe invalide** `format('ghcr.io/{0}/inventiv-agents', github.repository_owner)` → Doit être `${{ github.repository_owner }}`
- ⚠️ **Dépendance sur secrets** non vérifiés (peut échouer silencieusement)

#### 3. **Deploy Production** (`.github/workflows/deploy-prod.yml`)
- ⚠️ **Status**: Non testé (manuel uniquement)
- **Déclenchement**: `workflow_dispatch` uniquement
- **Pipeline**: Promote tag → `:prod` + `make prod-update`
- **Même problème** que staging (heredoc + format)

#### 4. **GHCR** (`.github/workflows/ghcr.yml`)
- ✅ **Status**: Opérationnel (build ARM64 sur tags `v*`)
- **Déclenchement**: Push tag `v*` + `workflow_dispatch`
- **Pipeline**: Build ARM64 + tag version

---

## 🔧 Tooling (Makefile)

### Commandes CI locale
- ✅ `make ci-fast`: fmt-check + clippy + test + npm ci + lint + build
- ✅ `make security-check`: Détecte les clés privées dans les fichiers trackés
- ✅ `make fmt-check`, `make clippy`, `make test`
- ✅ `make ui-lint`, `make ui-build`

### Commandes déploiement
- ✅ `make stg-update`: Pull + renew cert + up -d (staging)
- ✅ `make prod-update`: Pull + renew cert + up -d (production)
- ✅ `make stg-status`, `make stg-logs`
- ✅ `make images-promote-stg`, `make images-promote-prod`

### Scripts de déploiement
- ✅ `scripts/deploy_remote.sh`: Orchestre le déploiement SSH
- ✅ `scripts/remote_bootstrap.sh`: Bootstrap VM (docker, compose, dirs)
- ✅ `scripts/remote_sync_secrets.sh`: Sync secrets vers VM
- ✅ `scripts/ssh_detect_user.sh`: Auto-détection user SSH

---

## 🗄️ Base de données

### Migrations SQL
- ✅ Baseline: `00000000000000_baseline.sql` (modifié)
- ✅ Nouvelle: `20260108000001_add_user_account_plan_and_wallet.sql`
- ✅ Nouvelle: `20260108000002_add_org_subscription_plan_and_wallet.sql`
- ✅ Nouvelle: `20260108000003_add_instances_organization_id.sql`
- ✅ Nouvelle: `20260108000004_add_instances_double_activation.sql`

**Note**: Migrations non commitées → Risque de divergence staging/prod si déployées sans commit.

---

## 🔐 Secrets GitHub (environments)

### Environment `staging` requis
- ✅ `STG_REMOTE_HOST` (ex: `51.159.133.239`)
- ✅ `STG_SECRETS_DIR` (ex: `/opt/inventiv/secrets-staging`)
- ✅ `STG_SSH_PRIVATE_KEY` (clé privée SSH multi-ligne)
- ✅ `STG_POSTGRES_PASSWORD`
- ✅ `STG_WORKER_AUTH_TOKEN`
- ✅ `STG_ROOT_DOMAIN` (ex: `inventiv-agents.fr`)
- ✅ `STG_FRONTEND_DOMAIN` (ex: `studio-stg.inventiv-agents.fr`)
- ✅ `STG_API_DOMAIN` (ex: `api-stg.inventiv-agents.fr`)
- ✅ `STG_ACME_EMAIL`

**Optionnels**:
- `STG_REMOTE_PORT` (défaut: 22)
- `STG_REMOTE_USER` (défaut: `ubuntu`)
- `IMAGE_REPO` (défaut: `ghcr.io/<owner>/inventiv-agents`)
- `GHCR_USERNAME` (défaut: `<owner>`)
- `STG_PROVIDER`, `STG_SCALEWAY_PROJECT_ID`, `STG_RUST_LOG`, etc.

### Environment `production` requis
- Mêmes secrets avec préfixe `PROD_`

---

## 🐛 Problèmes identifiés

### 1. **CRITIQUE**: Variables GitHub Actions non substituées dans `deploy-staging.yml`

**Fichier**: `.github/workflows/deploy-staging.yml` ligne 137

**Problème**:
```yaml
cat > env/staging.env <<'EOF'  # ❌ Single quotes = pas de substitution
  REMOTE_HOST=${{ secrets.STG_REMOTE_HOST }}  # ❌ Reste littéral
```

**Solution**: Utiliser `EOF` sans quotes ou `"EOF"`:
```yaml
cat > env/staging.env <<EOF  # ✅ Pas de quotes = substitution activée
  REMOTE_HOST=${{ secrets.STG_REMOTE_HOST }}
```

### 2. **CRITIQUE**: Syntaxe invalide `format()` dans GitHub Actions

**Fichier**: `.github/workflows/deploy-staging.yml` ligne 145

**Problème**:
```yaml
IMAGE_REPO=${{ secrets.IMAGE_REPO || format('ghcr.io/{0}/inventiv-agents', github.repository_owner) }}
# ❌ format() n'existe pas dans GitHub Actions
```

**Solution**: Utiliser directement `github.repository_owner`:
```yaml
IMAGE_REPO=${{ secrets.IMAGE_REPO || format('ghcr.io/{0}/inventiv-agents', github.repository_owner) }}
# ✅ Devient:
IMAGE_REPO=${{ secrets.IMAGE_REPO || format('ghcr.io/{0}/inventiv-agents', github.repository_owner) }}
```

**Correction complète**:
```yaml
IMAGE_REPO=${{ secrets.IMAGE_REPO || format('ghcr.io/{0}/inventiv-agents', github.repository_owner) }}
# ✅ Devient:
IMAGE_REPO=${{ secrets.IMAGE_REPO || format('ghcr.io/{0}/inventiv-agents', github.repository_owner) }}
```

### 3. **Moyen**: Migrations non commitées

**Risque**: Si déployé sans commit, staging/prod auront des schémas DB différents.

**Solution**: Commiter les migrations avant déploiement.

### 4. **Mineur**: Pas de validation des secrets avant déploiement

**Risque**: Le workflow échoue silencieusement si un secret manque.

**Solution**: Ajouter une étape de validation des secrets requis.

---

## ✅ Plan d'action pour fixer CI/CD

### Étape 1: Corriger les workflows (URGENT)
1. ✅ Fix heredoc dans `deploy-staging.yml` (retirer single quotes)
2. ✅ Fix syntaxe `format()` → utiliser `github.repository_owner` directement
3. ✅ Appliquer les mêmes fixes à `deploy-prod.yml`

### Étape 2: Valider les secrets GitHub
1. Vérifier que l'environment `staging` existe
2. Vérifier que tous les secrets requis sont présents
3. Tester la connexion SSH depuis GitHub Actions (ajouter un step de test)

### Étape 3: Tester le déploiement
1. Commiter les migrations SQL
2. Push sur `main` pour déclencher staging
3. Vérifier les logs GitHub Actions
4. Vérifier que les containers sont à jour sur la VM

### Étape 4: Provisionner staging/prod
1. Vérifier l'état des VMs (`make stg-status`, `make prod-status`)
2. Si nécessaire, reprovisionner (`make stg-rebuild`)
3. Relancer un déploiement de test

---

## 📝 Notes

- Les VMs Scaleway sont **ARM64** → Les builds doivent utiliser `--platform linux/arm64`
- Les images sont promues par **digest** (immutabilité garantie)
- Le workflow staging est **non-cancellable** (`cancel-in-progress: false`) pour éviter d'interrompre un déploiement

---

## 🔗 Références

- `docs/CI_CD.md`: Documentation CI/CD
- `docs/DEPLOYMENT_STAGING.md`: Guide déploiement staging
- `.github/workflows/`: Tous les workflows GitHub Actions
- `Makefile`: Commandes make disponibles

