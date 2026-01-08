#!/bin/bash
# Script pour lancer les tests de sessions (Phase 1)

set -e

echo "🧪 Lancement des tests Phase 1 : Architecture Sessions"
echo ""

# Vérifier que DB et Redis sont disponibles
if ! docker ps | grep -q "postgres\|redis"; then
    echo "⚠️  Containers Docker non démarrés. Démarrage..."
    make up db redis 2>&1 | grep -E "(Creating|Starting|Started)" || true
    echo "⏳ Attente démarrage containers..."
    sleep 5
fi

# Variables d'environnement
export TEST_DATABASE_URL="${TEST_DATABASE_URL:-postgresql://postgres:password@localhost:5432/inventiv_test}"
export TEST_REDIS_URL="${TEST_REDIS_URL:-redis://localhost:6379/1}"
export JWT_SECRET="test-secret-key-for-testing-only"
export JWT_ISSUER="inventiv-api"

echo "📊 Configuration :"
echo "  TEST_DATABASE_URL: $TEST_DATABASE_URL"
echo "  TEST_REDIS_URL: $TEST_REDIS_URL"
echo ""

cd inventiv-api

echo "🔍 Tests unitaires (auth.rs)..."
echo ""
cargo test --lib auth::tests -- --nocapture 2>&1 | grep -E "(test|PASSED|FAILED|error)" || true

echo ""
echo "🔍 Tests d'intégration (auth_test.rs)..."
echo ""
cargo test --test auth_test -- --nocapture 2>&1 | grep -E "(test|PASSED|FAILED|error)" || true

echo ""
echo "✅ Tests terminés"
