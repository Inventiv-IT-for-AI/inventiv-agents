# Test Scaleway L4-1-24G Provisioning

Scripts de test indépendants pour valider la séquence d'appels API Scaleway qui fonctionne réellement pour créer une instance L4-1-24G avec Block Storage.

## Scripts Disponibles

### 1. `test-scaleway` - Test de provisioning complet
Teste différentes séquences pour créer une instance L4-1-24G avec Block Storage.

### 2. `analyze-instance` - Analyse d'une instance existante
Analyse une instance créée manuellement via la console pour comparer sa configuration avec ce que notre code génère.

## Prérequis

```bash
export SCALEWAY_SECRET_KEY="your-secret-key"
export SCALEWAY_PROJECT_ID="your-project-id"
export SCALEWAY_ZONE="fr-par-2"  # Optionnel, défaut: fr-par-2
export SCALEWAY_ACCESS_KEY="your-access-key"  # Pour CLI operations
export SCALEWAY_ORGANIZATION_ID="your-org-id"  # Pour CLI operations
```

## Utilisation

### Tester le provisioning
```bash
cd test-scaleway
cargo run --bin test-scaleway
```

### Analyser une instance existante
```bash
# Après avoir créé une instance via la console Scaleway
cargo run --bin analyze-instance <server-id>

# Ou avec variable d'environnement
export SCALEWAY_SERVER_ID="your-server-id"
cargo run --bin analyze-instance
```

## Objectif

Trouver la séquence d'appels API la plus simple qui permet de :
1. Créer une instance L4-1-24G
2. Attacher un Block Storage de 200GB
3. Démarrer l'instance
4. Accéder à l'instance via SSH
5. Vérifier que le Block Storage est accessible

## Résultats des Tests

Voir `SCENARIO_SUMMARY.md` pour les résultats détaillés des tests effectués.

## Prochaines Étapes

Une fois qu'on a validé ce qui fonctionne, on adaptera le code du provider Scaleway pour utiliser la séquence validée.
