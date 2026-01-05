# Centralisation de la logique de recommandation de taille de disque

## Objectif

**Source unique de vérité** pour la taille de disque recommandée selon le modèle choisi, utilisée par :
- **Frontend** : Affichage cohérent dans la modale de création d'instance
- **Orchestrator** : Allocation de la bonne taille lors du provisioning
- **Providers** : Création des volumes avec la taille appropriée

## Architecture

### Logique centralisée : `inventiv-common/src/worker_storage.rs`

Fonction `recommended_data_volume_gb(model_id: &str, default_gb: i64) -> Option<i64>` qui :
- Calcule la taille recommandée basée sur le `model_id` (HuggingFace repo ID)
- Prend en compte les variables d'environnement :
  - `WORKER_DATA_VOLUME_GB` : Force une taille fixe pour tous les workers
  - `WORKER_DATA_VOLUME_GB_DEFAULT` : Taille par défaut pour modèles inconnus
- Logique heuristique basée sur la taille du modèle :
  - 0.5B/0.6B → 50GB
  - 1B/1.5B/2B → 70GB
  - 7B/8B → 100GB
  - 12B/13B/14B → 120GB
  - 24B/27B/30B/32B → 180GB
  - 70B/72B → 450GB
  - Autres → `default_gb` (typiquement 200GB)

### Endpoint API : `GET /models/{id}/recommended-data-volume`

Expose la logique centralisée pour le frontend :
- Paramètres :
  - `id` : UUID du modèle
  - `provider_id` (optionnel) : Pour utiliser les settings spécifiques du provider
- Retourne :
  - `recommended_data_volume_gb` : Valeur calculée par la logique centralisée
  - `stored_data_volume_gb` : Valeur stockée dans la DB (peut être NULL)
  - `default_gb` : Valeur par défaut utilisée

### Utilisation dans le Frontend

Dans `CreateInstanceModal.tsx` :
- Appelle l'endpoint quand un modèle est sélectionné
- Affiche la valeur recommandée calculée dynamiquement
- Affiche "(recommandé)" si la valeur vient du calcul et non de la DB
- Priorité : `data_volume_gb` de la DB si présent, sinon valeur recommandée

### Utilisation dans l'Orchestrator

Dans `inventiv-orchestrator/src/services.rs` :
- Utilise `inventiv_common::worker_storage::recommended_data_volume_gb()` lors du provisioning
- Prend en compte les settings du provider pour `default_gb`
- Crée les volumes avec la taille calculée

## Avantages

1. **Source unique de vérité** : Un seul endroit à modifier si la logique change
2. **Cohérence** : Frontend et Orchestrator utilisent la même logique
3. **Flexibilité** : Variables d'environnement et settings provider pour override
4. **Pas de migration nécessaire** : Les instances déjà provisionnées gardent leur taille
5. **Affichage cohérent** : Le frontend montre toujours la valeur qui sera utilisée

## Fichiers modifiés

- ✅ `inventiv-common/src/worker_storage.rs` (nouveau, logique centralisée)
- ✅ `inventiv-common/src/lib.rs` (export du module)
- ✅ `inventiv-orchestrator/src/services.rs` (utilise `inventiv_common::worker_storage`)
- ✅ `inventiv-api/src/main.rs` (endpoint `/models/{id}/recommended-data-volume`)
- ✅ `inventiv-frontend/src/components/instances/CreateInstanceModal.tsx` (appel de l'endpoint et affichage)

