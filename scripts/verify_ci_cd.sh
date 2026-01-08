#!/usr/bin/env bash
# Script de vérification rapide de la CI/CD

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

echo "🔍 Vérification CI/CD - Inventiv Agents"
echo "========================================"
echo ""

# 1. Vérifier les fichiers workflows
echo "📁 Vérification des workflows..."
WORKFLOWS=(
  ".github/workflows/ci.yml"
  ".github/workflows/deploy-staging.yml"
  ".github/workflows/deploy-prod.yml"
  ".github/workflows/ghcr.yml"
)

MISSING=0
for wf in "${WORKFLOWS[@]}"; do
  if [[ -f "$wf" ]]; then
    echo "  ✅ $wf"
  else
    echo "  ❌ $wf (MANQUANT)"
    MISSING=1
  fi
done

if [[ $MISSING -eq 1 ]]; then
  echo ""
  echo "❌ Certains workflows sont manquants"
  exit 1
fi

# 2. Vérifier la syntaxe YAML
echo ""
echo "🔤 Vérification syntaxe YAML..."
if command -v python3 >/dev/null 2>&1; then
  YAML_ERROR=0
  for wf in "${WORKFLOWS[@]}"; do
    if python3 -c "import sys,yaml; yaml.safe_load(open('$wf'))" >/dev/null 2>&1; then
      echo "  ✅ $wf"
    else
      echo "  ❌ $wf (erreur syntaxe)"
      YAML_ERROR=1
    fi
  done
  if [[ $YAML_ERROR -eq 0 ]]; then
    echo "  ✅ Tous les workflows YAML sont valides"
  else
    echo "  ❌ Erreurs de syntaxe YAML détectées"
    exit 1
  fi
else
  echo "  ⚠️  Python3 non disponible, skip validation YAML"
fi

# 3. Vérifier le remote GitHub
echo ""
echo "🔗 Vérification remote GitHub..."
REMOTE_URL=$(git remote get-url origin 2>/dev/null || echo "")
if [[ "$REMOTE_URL" == *"github.com"* ]]; then
  echo "  ✅ Remote GitHub détecté: ${REMOTE_URL}"
  OWNER_REPO=$(echo "$REMOTE_URL" | sed -E 's|.*github.com[:/]([^/]+/[^/]+)(\.git)?$|\1|')
  echo "  📦 Repo: ${OWNER_REPO}"
else
  echo "  ⚠️  Remote GitHub non détecté (ou non configuré)"
fi

# 4. Vérifier les secrets locaux (si env files existent)
echo ""
echo "🔐 Vérification configuration locale..."
if [[ -f "env/staging.env" ]]; then
  echo "  ✅ env/staging.env existe"
  if grep -q "REMOTE_HOST=" env/staging.env; then
    REMOTE_HOST=$(grep "^REMOTE_HOST=" env/staging.env | cut -d= -f2)
    echo "    REMOTE_HOST=${REMOTE_HOST}"
  fi
else
  echo "  ⚠️  env/staging.env manquant (créer depuis env/staging.env.example)"
fi

if [[ -f "env/prod.env" ]]; then
  echo "  ✅ env/prod.env existe"
else
  echo "  ⚠️  env/prod.env manquant (créer depuis env/prod.env.example)"
fi

# 5. Vérifier les images GHCR (si connecté)
echo ""
echo "🐳 Vérification images GHCR..."
if command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1; then
  if docker buildx imagetools inspect ghcr.io/inventiv-it-for-ai/inventiv-agents/inventiv-api:staging >/dev/null 2>&1; then
    echo "  ✅ Image :staging existe sur GHCR"
    DIGEST=$(docker buildx imagetools inspect ghcr.io/inventiv-it-for-ai/inventiv-agents/inventiv-api:staging --format '{{json .}}' 2>/dev/null | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('manifests',[{}])[0].get('digest','unknown')[:16])" 2>/dev/null || echo "unknown")
    echo "    Digest: ${DIGEST}"
  else
    echo "  ⚠️  Image :staging non trouvée sur GHCR (normal si jamais déployé)"
  fi
else
  echo "  ⚠️  Docker non disponible, skip vérification images"
fi

# 6. Vérifier les tags Git
echo ""
echo "🏷️  Vérification tags Git..."
LATEST_TAG=$(git tag -l "v*" | sort -V | tail -1 || echo "")
if [[ -n "$LATEST_TAG" ]]; then
  echo "  ✅ Dernier tag: ${LATEST_TAG}"
  if git ls-remote --tags origin | grep -q "refs/tags/${LATEST_TAG}"; then
    echo "    ✅ Tag poussé sur GitHub"
  else
    echo "    ⚠️  Tag non poussé (git push origin ${LATEST_TAG})"
  fi
else
  echo "  ⚠️  Aucun tag v* trouvé"
fi

# 7. Vérifier la CI locale
echo ""
echo "🧪 Test CI locale (make ci-fast)..."
if make -n ci-fast >/dev/null 2>&1; then
  echo "  ✅ Makefile target 'ci-fast' existe"
  echo "  💡 Pour tester: make ci-fast"
else
  echo "  ⚠️  Target 'ci-fast' non trouvé dans Makefile"
fi

# Résumé
echo ""
echo "========================================"
echo "✅ Vérification terminée"
echo ""
echo "📋 Prochaines étapes:"
echo ""
echo "1. Vérifier sur GitHub:"
echo "   https://github.com/${OWNER_REPO}/actions"
echo ""
echo "2. Vérifier les secrets GitHub:"
echo "   Settings → Secrets and variables → Actions → Environments"
echo ""
echo "3. Tester la CI:"
echo "   make ci-fast"
echo ""
echo "4. Déclencher un déploiement staging:"
echo "   git push origin main"
echo ""

