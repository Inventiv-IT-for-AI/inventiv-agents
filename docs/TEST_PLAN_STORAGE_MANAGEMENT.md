# Plan de Test - Gestion des Volumes (Storage Management)

**Date**: 2026-01-03  
**Objectif**: Valider méthodiquement que la gestion des volumes fonctionne correctement avec Scaleway avant de déclarer que tout est fixé.

---

## ⚠️ Principe

**Tester d'abord, corriger ensuite, documenter seulement après validation.**

---

## Phase 0 : Préparation et vérification de l'environnement

### 0.1 Vérifier l'environnement de test disponible

**Questions à répondre**:
- [ ] Stack locale fonctionne (`make up`)?
- [ ] Credentials Scaleway configurés dans `env/dev.env`?
- [ ] Accès SSH à une VM Scaleway de test disponible?
- [ ] API Scaleway accessible depuis l'environnement local?

**Commandes de vérification**:
```bash
# Vérifier stack locale
make up
docker compose ps

# Vérifier credentials Scaleway
grep -E "SCALEWAY|SCW" env/dev.env

# Vérifier accès API Scaleway (si credentials présents)
# TODO: Créer script de test d'accès API Scaleway
```

### 0.2 Vérifier l'état actuel du code

**Avant toute modification**:
- [ ] Compiler le code actuel (`cargo check`)
- [ ] Vérifier que les modifications précédentes compilent
- [ ] Comprendre le comportement actuel (sans mes modifications)

**Commandes**:
```bash
cargo check --workspace
cargo build --workspace
```

### 0.3 Créer un script de test manuel

**Objectif**: Script qui permet de tester étape par étape avec vérifications explicites.

**Structure proposée**:
```bash
#!/bin/bash
# scripts/test_storage_management_scaleway.sh

set -euo pipefail

# Variables
INSTANCE_ID=""
VOLUME_BOOT_ID=""
VOLUME_DATA_ID=""

# Fonctions de vérification
check_api_response() { ... }
check_db_state() { ... }
check_scaleway_api() { ... }
check_vm_state() { ... }
```

---

## Phase 1 : Test de création d'instance (sans mes modifications)

### 1.1 Créer une instance Scaleway via API

**Objectif**: Observer le comportement actuel avant modifications.

**Étapes**:
1. Créer instance via `POST /deployments`
2. Observer les `action_logs`
3. Vérifier dans DB (`instance_volumes`)
4. Vérifier dans Scaleway (volumes créés)

**Vérifications**:
- [ ] Instance créée dans Scaleway?
- [ ] Volume boot créé automatiquement par Scaleway?
- [ ] Volume boot tracké dans `instance_volumes`?
- [ ] `storage_count` et `storage_sizes_gb` corrects dans API?
- [ ] Volume data créé (si `data_volume_gb > 0`)?
- [ ] Volume data attaché?

**Commandes**:
```bash
# Créer instance
curl -X POST http://localhost:8003/deployments \
  -H "Content-Type: application/json" \
  -H "Cookie: session=..." \
  -d '{
    "instance_type_id": "...",
    "zone_id": "...",
    "model_id": "..."
  }'

# Vérifier instance
curl http://localhost:8003/instances/{id} \
  -H "Cookie: session=..."

# Vérifier DB
docker compose exec db psql -U postgres -d llminfra -c \
  "SELECT * FROM instance_volumes WHERE instance_id = '...'"

# Vérifier Scaleway (via API ou console)
# TODO: Script pour lister volumes Scaleway
```

### 1.2 Documenter les problèmes observés

**Format**:
- Problème observé
- Comportement attendu
- Comportement réel
- Preuve (logs, DB, API Scaleway)

---

## Phase 2 : Test avec modifications (si Phase 1 confirme les problèmes)

### 2.1 Appliquer les modifications une par une

**Principe**: Une modification à la fois, tester après chaque modification.

**Modifications à tester**:
1. `create_volume` dans Scaleway provider
2. `attach_volume` dans Scaleway provider
3. Découverte volumes boot après `PROVIDER_CREATE`
4. Tracking volumes dans `instance_volumes`

### 2.2 Test après chaque modification

**Pour chaque modification**:
- [ ] Code compile
- [ ] Test unitaire (si applicable)
- [ ] Test d'intégration (créer instance, vérifier volumes)
- [ ] Vérifier logs orchestrator
- [ ] Vérifier DB
- [ ] Vérifier API Scaleway

---

## Phase 3 : Test de terminaison

### 3.1 Terminer l'instance

**Étapes**:
1. Terminer instance via `DELETE /instances/{id}`
2. Observer `action_logs`
3. Vérifier dans DB (`instance_volumes.deleted_at`)
4. Vérifier dans Scaleway (volumes supprimés)

**Vérifications**:
- [ ] Instance supprimée dans Scaleway?
- [ ] Volume boot supprimé dans Scaleway?
- [ ] Volume data supprimé dans Scaleway (si `delete_on_terminate=true`)?
- [ ] `instance_volumes` marqués comme supprimés?

---

## Phase 4 : Tests de cas limites

### 4.1 Instance sans volume data

**Scénario**: Créer instance avec `data_volume_gb=0` ou `null`

**Vérifications**:
- [ ] Volume boot tracké
- [ ] Pas de volume data créé
- [ ] `storage_count=1` (boot seulement)

### 4.2 Instance avec volume data personnalisé

**Scénario**: Créer instance avec `data_volume_gb=500`

**Vérifications**:
- [ ] Volume boot tracké
- [ ] Volume data de 500GB créé
- [ ] Volume data attaché
- [ ] `storage_count=2`

### 4.3 Volume data avec `delete_on_terminate=false`

**Scénario**: Créer instance avec volume data persistant

**Vérifications**:
- [ ] Volume data créé et attaché
- [ ] À la terminaison: volume boot supprimé, volume data conservé

---

## Phase 5 : Validation finale

### 5.1 Test complet end-to-end

**Scénario**: Cycle complet création → utilisation → terminaison

**Vérifications**:
- [ ] Tous les volumes trackés correctement
- [ ] API retourne `storage_count` et `storage_sizes_gb` corrects
- [ ] Terminaison supprime tous les volumes appropriés
- [ ] Aucun volume orphelin dans Scaleway

### 5.2 Documentation des résultats

**Format**:
- Tests effectués
- Résultats observés
- Problèmes identifiés
- Solutions appliquées
- Tests de régression

---

## Outils nécessaires

### Scripts à créer

1. **`scripts/test_scaleway_api_access.sh`**
   - Vérifier credentials Scaleway
   - Tester connexion API Scaleway
   - Lister instances/volumes existants

2. **`scripts/test_storage_management.sh`**
   - Script principal de test
   - Appels API
   - Vérifications DB
   - Vérifications Scaleway

3. **`scripts/verify_volumes_in_scaleway.sh`**
   - Lister volumes pour une instance
   - Vérifier taille, type, état
   - Comparer avec DB

### Endpoints API à utiliser

- `POST /deployments` - Créer instance
- `GET /instances/{id}` - Détails instance (avec `storages[]`)
- `GET /instances` - Liste instances (avec `storage_count`, `storage_sizes_gb`)
- `DELETE /instances/{id}` - Terminer instance
- `GET /action_logs?instance_id={id}` - Logs d'actions

### Requêtes DB utiles

```sql
-- Vérifier volumes trackés pour une instance
SELECT * FROM instance_volumes 
WHERE instance_id = '...' 
ORDER BY is_boot DESC, created_at;

-- Vérifier storage_count et sizes
SELECT 
  instance_id,
  COUNT(*) as storage_count,
  ARRAY_AGG(size_bytes / 1000000000 ORDER BY size_bytes) as sizes_gb
FROM instance_volumes
WHERE instance_id = '...' AND deleted_at IS NULL
GROUP BY instance_id;
```

---

## Questions à résoudre avant de commencer

1. **Environnement de test**:
   - [ ] Stack locale fonctionne?
   - [ ] Credentials Scaleway disponibles?
   - [ ] VM Scaleway de test disponible?

2. **Accès Scaleway**:
   - [ ] API Scaleway accessible depuis local?
   - [ ] Console Scaleway accessible pour vérifications manuelles?
   - [ ] Scripts existants pour interagir avec Scaleway?

3. **Données de test**:
   - [ ] Zone Scaleway à utiliser (ex: `fr-par-2`)?
   - [ ] Type d'instance à utiliser (ex: `RENDER-S`)?
   - [ ] Modèle à utiliser (ex: `Qwen/Qwen2.5-7B-Instruct`)?

4. **Outils de vérification**:
   - [ ] Accès DB pour requêtes directes?
   - [ ] Accès logs orchestrator?
   - [ ] Outils pour vérifier Scaleway (API, console, CLI)?

---

## Prochaines étapes

1. **Répondre aux questions ci-dessus**
2. **Créer les scripts de test nécessaires**
3. **Exécuter Phase 0 (vérification environnement)**
4. **Exécuter Phase 1 (test comportement actuel)**
5. **Documenter résultats avant toute modification**
6. **Appliquer modifications une par une avec tests**

---

## Notes importantes

- **Ne pas modifier le code avant d'avoir testé le comportement actuel**
- **Documenter chaque étape de test**
- **Valider chaque modification avant de passer à la suivante**
- **Tester les cas limites et erreurs**
- **Vérifier la propreté (pas de volumes orphelins)**

