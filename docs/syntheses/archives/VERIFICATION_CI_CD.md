# Guide de Vérification CI/CD

## 🔍 Vérification rapide (5 minutes)

### 1. Vérifier que les workflows existent

```bash
# Vérifier les fichiers workflows
ls -la .github/workflows/

# Devrait afficher :
# - ci.yml
# - deploy-staging.yml
# - deploy-prod.yml
# - ghcr.yml
```

### 2. Vérifier la syntaxe YAML

```bash
# Valider la syntaxe YAML (nécessite Python)
python3 -c "import sys,yaml; [yaml.safe_load(open(f)) for f in ['.github/workflows/ci.yml', '.github/workflows/deploy-staging.yml', '.github/workflows/deploy-prod.yml', '.github/workflows/ghcr.yml']]; print('✅ YAML valide')"
```

### 3. Vérifier sur GitHub

1. **Aller sur GitHub** → Ton repo → Onglet **"Actions"**
2. **Vérifier que les workflows apparaissent** dans la sidebar gauche :
   - ✅ `CI`
   - ✅ `Deploy (staging)`
   - ✅ `Deploy (production)`
   - ✅ `GHCR (arm64 build + promote)`

---

## 🧪 Tests pratiques

### Test 1 : Vérifier que la CI fonctionne

#### Option A : Via un commit de test

```bash
# Créer un commit de test (ne modifie rien d'important)
echo "# Test CI" >> .github/test-ci.md
git add .github/test-ci.md
git commit -m "test: vérification CI"
git push origin main
```

**Vérifier** :
- GitHub → Actions → Voir le workflow `CI` qui se déclenche
- Attendre la fin (devrait être vert ✅)
- Vérifier les logs de chaque job (Rust + Frontend)

#### Option B : Via une Pull Request

1. Créer une branche : `git checkout -b test-ci`
2. Faire un petit changement (ex: commentaire dans un fichier)
3. Push : `git push origin test-ci`
4. Créer une PR sur GitHub
5. **Vérifier** : La CI devrait se déclencher automatiquement sur la PR

### Test 2 : Vérifier le workflow GHCR (build images)

#### Déclencher manuellement

1. **GitHub** → Actions → `GHCR (arm64 build + promote)`
2. Cliquer **"Run workflow"**
3. Sélectionner :
   - **Branch** : `main` (ou une branche avec un tag `v*`)
   - **promote_env** : laisser vide (ou `staging`)
   - **source_tag** : laisser vide (ou un tag existant comme `v0.4.9`)
4. Cliquer **"Run workflow"**

**Vérifier** :
- Le workflow démarre
- Le job `build_arm64` s'exécute (si un tag `v*` est présent)
- Les images sont buildées et pushées vers GHCR

#### Vérifier les images sur GHCR

```bash
# Lister les images disponibles
docker buildx imagetools inspect ghcr.io/inventiv-it-for-ai/inventiv-agents/inventiv-api:staging 2>&1 | head -10

# OU via l'interface GitHub
# GitHub → Ton repo → Packages → inventiv-agents/inventiv-api
# Vérifier les tags disponibles (:staging, :prod, :v0.4.9, etc.)
```

### Test 3 : Vérifier le déploiement staging

#### Prérequis
- ✅ Les secrets GitHub sont configurés (voir `docs/CI_CD.md`)
- ✅ La VM staging existe et est accessible via SSH

#### Déclencher le déploiement

**Option A : Automatique (push sur main)**
```bash
git push origin main
```

**Option B : Manuel**
1. GitHub → Actions → `Deploy (staging)`
2. **"Run workflow"** → Branch `main` → **"Run workflow"**

**Vérifier** :
1. **Sur GitHub Actions** :
   - Le workflow démarre
   - Le job `ci` passe ✅
   - Le job `deploy` :
     - Build images `linux/arm64` ✅
     - Push vers GHCR ✅
     - Promotion `:staging` ✅
     - Déploiement remote ✅

2. **Sur la VM staging** :
   ```bash
   # Se connecter à la VM
   ssh -i ./.ssh/llm-studio-key ubuntu@$(grep REMOTE_HOST env/staging.env | cut -d= -f2)
   
   # Vérifier les containers
   cd /opt/inventiv-agents/deploy
   docker compose ps
   
   # Vérifier les logs
   docker compose logs --tail=50 api
   ```

---

## 🔧 Vérification des secrets GitHub

### Secrets requis pour staging

1. **GitHub** → Ton repo → **Settings** → **Secrets and variables** → **Actions**
2. **Environments** → `staging`
3. **Vérifier que ces secrets existent** :
   - ✅ `STG_REMOTE_HOST` (ex: `51.159.133.239`)
   - ✅ `STG_SECRETS_DIR` (ex: `/opt/inventiv/secrets-staging`)
   - ✅ `STG_SSH_PRIVATE_KEY` (clé privée SSH, multi-ligne)
   - ✅ `STG_POSTGRES_PASSWORD`
   - ✅ `STG_WORKER_AUTH_TOKEN`
   - ✅ `STG_ROOT_DOMAIN` (ex: `inventiv-agents.fr`)
   - ✅ `STG_FRONTEND_DOMAIN` (ex: `studio-stg.inventiv-agents.fr`)
   - ✅ `STG_API_DOMAIN` (ex: `api-stg.inventiv-agents.fr`)
   - ✅ `STG_ACME_EMAIL`

### Secrets requis pour production

Mêmes secrets avec préfixe `PROD_` dans l'environment `production`.

---

## 🐛 Dépannage

### Le workflow ne se déclenche pas

**Symptôme** : Push sur `main` mais aucun workflow ne démarre

**Vérifications** :
1. ✅ Les fichiers `.github/workflows/*.yml` sont bien commités
2. ✅ La syntaxe YAML est valide
3. ✅ Les permissions GitHub Actions sont activées (Settings → Actions → General)

**Solution** :
```bash
# Vérifier que les workflows sont trackés
git ls-files .github/workflows/

# Si manquants, les ajouter
git add .github/workflows/
git commit -m "chore: add CI/CD workflows"
git push origin main
```

### Le workflow échoue au build

**Symptôme** : Le job `build_arm64` échoue

**Vérifications** :
1. ✅ Les Dockerfiles existent (`Dockerfile.rust.prod`, `inventiv-frontend/Dockerfile`)
2. ✅ Les dépendances sont correctes (`Cargo.toml`, `package.json`)
3. ✅ Les secrets GHCR sont configurés (`GITHUB_TOKEN` est automatique)

**Solution** :
- Vérifier les logs GitHub Actions pour l'erreur exacte
- Tester le build localement :
  ```bash
  docker buildx build --platform linux/arm64 -f Dockerfile.rust.prod --build-arg SERVICE_NAME=inventiv-api -t test:latest .
  ```

### Le workflow échoue au déploiement

**Symptôme** : Le build passe mais `make stg-update` échoue

**Vérifications** :
1. ✅ Les secrets SSH sont corrects (`STG_SSH_PRIVATE_KEY`)
2. ✅ La VM est accessible : `ssh -i .ssh/llm-studio-key ubuntu@<STG_REMOTE_HOST>`
3. ✅ Les secrets sont sync sur la VM : `make stg-secrets-sync`

**Solution** :
```bash
# Tester la connexion SSH
ssh -i ./.ssh/llm-studio-key ubuntu@$(grep REMOTE_HOST env/staging.env | cut -d= -f2) "echo OK"

# Tester le déploiement manuellement
make stg-update
```

### Les images ne sont pas trouvées

**Symptôme** : `ERROR: ghcr.io/.../inventiv-api:v0.4.9: not found`

**Cause** : L'image n'a pas encore été buildée pour ce tag

**Solution** :
1. Vérifier que le tag existe : `git tag -l "v0.4.9"`
2. Vérifier que le tag est poussé : `git ls-remote --tags origin | grep v0.4.9`
3. Attendre que le workflow `ghcr.yml` termine (ou le déclencher manuellement)
4. OU utiliser le SHA du dernier commit :
   ```bash
   SHA=$(git rev-parse --short=12 HEAD)
   make images-promote-stg IMAGE_TAG=$SHA
   ```

---

## 📊 Monitoring continu

### Badges GitHub Actions

Ajouter dans ton `README.md` :

```markdown
[![CI](https://github.com/<owner>/<repo>/actions/workflows/ci.yml/badge.svg)](https://github.com/<owner>/<repo>/actions/workflows/ci.yml)
[![Deploy Staging](https://github.com/<owner>/<repo>/actions/workflows/deploy-staging.yml/badge.svg)](https://github.com/<owner>/<repo>/actions/workflows/deploy-staging.yml)
```

### Vérifier l'historique

1. **GitHub** → Actions
2. Voir tous les runs récents
3. Filtrer par workflow (`CI`, `Deploy (staging)`, etc.)
4. Vérifier les statuts (✅ vert = OK, ❌ rouge = échec)

---

## ✅ Checklist de vérification complète

- [ ] Les fichiers workflows existent (`.github/workflows/*.yml`)
- [ ] La syntaxe YAML est valide
- [ ] Les workflows apparaissent sur GitHub Actions
- [ ] La CI se déclenche sur les PRs
- [ ] La CI passe avec succès (fmt/clippy/test + frontend)
- [ ] Les secrets GitHub sont configurés (staging + production)
- [ ] Le workflow `ghcr.yml` build les images (test avec un tag `v*`)
- [ ] Les images apparaissent sur GHCR (Packages)
- [ ] Le workflow `deploy-staging.yml` déploie automatiquement (push sur main)
- [ ] La VM staging est accessible et les containers tournent
- [ ] Les logs de déploiement sont propres (pas d'erreurs)

---

## 🚀 Test complet end-to-end

Pour tester toute la chaîne :

```bash
# 1. Vérifier l'état local
make ci-fast  # Devrait passer ✅

# 2. Créer un commit de test
echo "# Test E2E $(date)" >> .github/test-e2e.md
git add .github/test-e2e.md
git commit -m "test: vérification E2E CI/CD"
git push origin main

# 3. Surveiller sur GitHub
# GitHub → Actions → Voir les workflows qui se déclenchent

# 4. Vérifier le résultat
# - CI passe ✅
# - Deploy staging démarre ✅
# - Images buildées ✅
# - Déploiement réussi ✅

# 5. Vérifier sur la VM
make stg-status
make stg-logs
```

---

**Dernière mise à jour** : 7 janvier 2026

