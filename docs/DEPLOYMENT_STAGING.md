# Guide de Déploiement Staging

## 🚀 Méthode 1 : Automatique (push sur `main`)

**Le plus simple** : chaque push sur `main` déclenche automatiquement le déploiement staging.

### Étapes

```bash
# 1. Commiter tes changements
git add .
git commit -m "feat: ..."

# 2. Push sur main
git push origin main
```

**Ce qui se passe automatiquement** :
1. ✅ CI s'exécute (fmt/clippy/test + frontend lint/build)
2. ✅ Build images `linux/arm64` avec tag `<sha12>` (ex: `a1b2c3d4e5f6`)
3. ✅ Push vers GHCR
4. ✅ Promotion `<sha12>` → `:staging` (même digest)
5. ✅ `make stg-update` sur la VM staging (pull + renew cert + up -d)

**Avantages** :
- ✅ Automatique, pas d'intervention manuelle
- ✅ Trace complète (chaque commit = déploiement)
- ✅ Rollback facile (promouvoir un autre `<sha12>`)

---

## 🏷️ Méthode 2 : Avec tag version (recommandé pour releases)

Pour créer une **release versionnée** (`v0.4.9`, `v0.5.0`, etc.).

### Étapes

#### 1. Mettre à jour la version (si nécessaire)

```bash
# Vérifier la version actuelle
cat VERSION
# → 0.4.9

# Si besoin de changer la version
echo "0.4.10" > VERSION
git add VERSION
git commit -m "chore: bump version to 0.4.10"
```

#### 2. Créer et pousser le tag

```bash
# Créer le tag (doit commencer par 'v')
git tag v0.4.9

# Pousser le tag
git push origin v0.4.9
```

**Ce qui se passe automatiquement** :
- ✅ Workflow `ghcr.yml` se déclenche
- ✅ Build images `linux/arm64` avec tags `:v0.4.9` ET `:<sha12>`
- ✅ Push vers GHCR

#### 3. Promouvoir vers staging (2 options)

##### Option A : Via GitHub Actions UI (recommandé)

1. Aller sur GitHub → Actions → `GHCR (arm64 build + promote)`
2. Cliquer "Run workflow"
3. Sélectionner :
   - `promote_env`: `staging`
   - `source_tag`: `v0.4.9` (ou `<sha12>`)
4. Cliquer "Run workflow"

**Ce qui se passe** :
- ✅ Promotion `v0.4.9` → `:staging` (même digest)
- ⚠️ **Ne déploie PAS automatiquement** (il faut ensuite déclencher `deploy-staging.yml` manuellement ou faire `make stg-update` en local)

##### Option B : Via Makefile (local)

```bash
# Promouvoir un tag existant vers staging
make images-promote-stg IMAGE_TAG=v0.4.9

# OU promouvoir un SHA
make images-promote-stg IMAGE_TAG=a1b2c3d4e5f6

# Puis déployer
make stg-update
```

**Prérequis** :
- ✅ Être connecté à GHCR (`make ghcr-login`)
- ✅ Avoir `env/staging.env` configuré localement
- ✅ Avoir accès SSH à la VM staging

---

## 📋 Comparaison des méthodes

| Critère | Méthode 1 (push main) | Méthode 2 (tag version) |
|---------|----------------------|------------------------|
| **Automatisation** | ✅ 100% automatique | ⚠️ Promotion manuelle |
| **Déploiement** | ✅ Automatique | ⚠️ Manuel (`make stg-update`) |
| **Traçabilité** | SHA commit | Tag version |
| **Rollback** | Promouvoir autre SHA | Promouvoir autre tag |
| **Use case** | Développement continu | Releases versionnées |

---

## 🔄 Workflow recommandé

### Pour le développement quotidien
```bash
# Push sur main → déploiement automatique
git push origin main
```

### Pour une release
```bash
# 1. Finaliser les changements
git add .
git commit -m "feat: ..."
git push origin main

# 2. Attendre que le déploiement staging soit OK

# 3. Créer le tag de release
git tag v0.4.9
git push origin v0.4.9

# 4. Promouvoir vers staging (si besoin de réutiliser ce tag)
# Via GitHub Actions UI ou make images-promote-stg
```

---

## ⚠️ Points d'attention

1. **Secrets GitHub** : Les workflows utilisent des secrets (`STG_REMOTE_HOST`, `STG_SSH_PRIVATE_KEY`, etc.). Vérifier qu'ils sont configurés dans l'environment `staging`.

2. **Build ARM64** : Les images sont buildées en `linux/arm64` (compatible Scaleway). Le build prend ~10-15 minutes.

3. **Certificats** : Le workflow renouvelle automatiquement les certificats Let's Encrypt si nécessaire.

4. **Rollback** : Pour revenir en arrière, promouvoir un autre tag/SHA :
   ```bash
   make images-promote-stg IMAGE_TAG=<ancien-sha>
   make stg-update
   ```

---

## 🐛 Dépannage

### Le workflow ne se déclenche pas
- Vérifier que le push est bien sur `main`
- Vérifier les permissions GitHub Actions

### Le déploiement échoue
- Vérifier les logs GitHub Actions
- Vérifier la connectivité SSH (`make stg-status`)
- Vérifier que les secrets sont bien configurés

### Les images ne sont pas trouvées
- Vérifier que le build a réussi (onglet "Build & push images")
- Vérifier que `IMAGE_REPO` est correct dans les secrets
- Vérifier que le tag existe sur GHCR

