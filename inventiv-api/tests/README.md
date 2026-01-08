# Tests d'intégration et de non-régression

Ce dossier contient les tests d'intégration et de non-régression pour tous les endpoints de l'API.

## Types de tests

### 1. Tests unitaires (in-memory)

**Fichiers**: `auth_test.rs`, `instances_test.rs`, `deployments_test.rs`

Ces tests utilisent `axum-test` et `TestServer` pour créer une instance in-memory de l'API. Ils ne nécessitent pas de containers Docker mais nécessitent une base de données PostgreSQL et Redis accessibles sur `localhost`.

**Exécution**:
```bash
# Avec DB/Redis sur localhost
make test-integration

# Ou manuellement avec variables d'environnement
cd inventiv-api
TEST_DATABASE_URL="postgresql://postgres:password@localhost:5432/llminfra" \
TEST_REDIS_URL="redis://localhost:6379/0" \
cargo test --test auth_test
```

**Avantages**:
- Rapides
- Isolés (chaque test a sa propre instance)
- Pas besoin de démarrer les containers

**Inconvénients**:
- Ne testent pas l'API réelle dans le container
- Nécessitent DB/Redis sur localhost

### 2. Tests d'intégration E2E (contre containers Docker réels)

**Fichier**: `integration_e2e.rs`

Ces tests se connectent à l'API réelle dans les containers Docker via HTTP sur `http://localhost:8003`. Ils testent le comportement réel de l'API avec tous ses composants (DB, Redis, Orchestrator).

**Prérequis**:
```bash
# Démarrer les containers
make up db redis api orchestrator
```

**Exécution**:
```bash
# Tests E2E simples (containers déjà démarrés)
make test-e2e

# Tests E2E complets (démarre containers, teste, arrête)
make test-e2e-full
```

**Variables d'environnement**:
- `TEST_API_URL` : URL de l'API (défaut: `http://localhost:8003`)
- `TEST_ADMIN_EMAIL` : Email de l'admin (défaut: `admin@inventiv.local`)
- `TEST_ADMIN_PASSWORD` : Mot de passe de l'admin (défaut: lit depuis `/run/secrets/default_admin_password` ou `./deploy/secrets-dev/default_admin_password`)

**Avantages**:
- Testent l'API réelle avec tous ses composants
- Valident le comportement end-to-end
- Détectent les problèmes d'intégration entre services

**Inconvénients**:
- Plus lents (nécessitent les containers)
- Nécessitent que les containers soient démarrés

## Configuration

### Variables d'environnement pour tests unitaires

Les tests unitaires utilisent des variables d'environnement pour configurer la base de données et Redis :

- `TEST_DATABASE_URL` : URL de la base de données PostgreSQL de test (défaut: `postgresql://postgres:postgres@localhost:5432/inventiv_test`)
- `TEST_REDIS_URL` : URL de Redis de test (défaut: `redis://localhost:6379/1`)

### Compatibilité avec Docker Compose

Les tests peuvent utiliser les containers Docker en configurant les URLs :

```bash
# Pour utiliser les containers Docker
TEST_DATABASE_URL="postgresql://postgres:password@localhost:5432/llminfra" \
TEST_REDIS_URL="redis://localhost:6379/0" \
cargo test --test auth_test
```

**Note**: Les containers Docker exposent les ports sur `localhost`, donc les tests peuvent s'y connecter directement.

## Structure des tests

- `common/` : Utilitaires et helpers communs pour tous les tests
  - `mod.rs` : Helpers pour tests unitaires (TestServer)
  - `e2e.rs` : Helpers pour tests E2E (HTTP client)
- `auth_test.rs` : Tests pour les endpoints d'authentification (unitaires)
- `deployments_test.rs` : Tests pour les endpoints de déploiement (Mock uniquement, unitaires)
- `instances_test.rs` : Tests pour les endpoints d'instances (Mock uniquement, unitaires)
- `integration_e2e.rs` : Tests E2E contre l'API réelle dans Docker
- `organizations_test.rs` : Tests pour les endpoints d'organisations (à créer)
- `models_test.rs` : Tests pour les endpoints de modèles (à créer)
- `users_test.rs` : Tests pour les endpoints d'utilisateurs (à créer)
- `api_keys_test.rs` : Tests pour les endpoints d'API keys (à créer)
- `workbench_test.rs` : Tests pour les endpoints Workbench (à créer)

## Règles importantes

### Provider Mock uniquement

**TOUS les tests de provisioning d'instances DOIVENT utiliser uniquement le provider Mock** pour éviter les coûts cloud.

Les tests vérifient que :
1. Les déploiements avec `provider_code: "mock"` fonctionnent correctement
2. Les déploiements avec d'autres providers (ex: `scaleway`) sont rejetés dans les tests

## Exécution complète

```bash
# 1. Tests unitaires (rapides, nécessitent DB/Redis sur localhost)
make test-unit

# 2. Tests d'intégration (nécessitent DB/Redis sur localhost)
make test-integration

# 3. Tests E2E (nécessitent containers Docker démarrés)
make test-e2e

# 4. Tous les tests (unitaires + intégration)
make test-all

# 5. Tests E2E complets (démarre containers automatiquement)
make test-e2e-full
```

## Workflow recommandé

### Développement local

1. **Tests unitaires rapides** (pendant le développement) :
   ```bash
   make test-unit
   ```

2. **Tests d'intégration** (avant commit) :
   ```bash
   make up db redis  # Démarrer seulement DB et Redis
   make test-integration
   ```

3. **Tests E2E complets** (avant push) :
   ```bash
   make test-e2e-full  # Démarre tout, teste, arrête
   ```

### CI/CD

Les tests E2E peuvent être intégrés dans la CI/CD pour valider le comportement réel de l'API avec tous ses composants.
