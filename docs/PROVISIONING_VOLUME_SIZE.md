# Provisioning VM - Configuration Taille Disque

**Date**: 2025-01-08

## Modifications

### 1. Support de la taille du root volume

Le script `scripts/scw_instance_provision.sh` supporte maintenant la configuration de la taille du root volume lors du provisioning des VMs control-plane (staging/prod).

#### Configuration

Ajoutez dans vos fichiers `env/staging.env` ou `env/prod.env`:

```bash
# Scaleway VM configuration
SCW_ROOT_VOLUME_SIZE_GB=40  # Staging: 40GB
# SCW_ROOT_VOLUME_SIZE_GB=100  # Prod: 100GB

# Scaleway CLI credentials (REQUIRED pour redimensionner Block Storage)
# Ces valeurs sont lues depuis deploy/secrets/ (pas depuis .env pour la sécurité)
# Créez ces fichiers:
#   - deploy/secrets/scaleway_organization_id
#   - deploy/secrets/scaleway_access_key
# Obtenez les valeurs depuis: https://console.scaleway.com/iam/api-keys
```

**Important** : Les fichiers `deploy/secrets/scaleway_organization_id` et `deploy/secrets/scaleway_access_key` sont **requis** pour redimensionner les Block Storage via le CLI `scw`. Sans ces fichiers, le redimensionnement échouera.

**Note** : Pour le développement local, vous pouvez aussi définir ces variables dans `.env` (fichier gitignored).

#### Valeurs recommandées

- **Staging**: `SCW_ROOT_VOLUME_SIZE_GB=40` (40GB)
- **Production**: `SCW_ROOT_VOLUME_SIZE_GB=100` (100GB)
- **Par défaut**: Si non spécifié, Scaleway crée un volume de ~10GB (taille par défaut pour BASIC2 instances)

### 2. Fonctionnement

Le script tente d'abord de spécifier la taille du root volume lors de la création via l'API Scaleway (`root_volume.size`). Si cette approche n'est pas supportée ou si le volume créé est plus petit que la taille cible, le script:

**Pour les Block Storage (sbs_volume)** :
- Utilise **uniquement le CLI `scw`** (l'API Scaleway ne supporte pas le redimensionnement des Block Storage)
- Nécessite `SCALEWAY_ORGANIZATION_ID` et `SCALEWAY_ACCESS_KEY`
- Vérifie automatiquement que le redimensionnement est terminé

**Pour les volumes locaux (l_ssd)** :
- Utilise l'API Instance Scaleway

1. **Détecte** le volume root de l'instance
2. **Vérifie** la taille actuelle
3. **Arrête** l'instance si nécessaire (redimensionnement nécessite l'arrêt)
4. **Redimensionne** le volume via l'API Instance (`PATCH /volumes/{volume_id}`)
5. **Redémarre** l'instance après redimensionnement

### 3. Exemple d'utilisation

```bash
# Provisionner staging avec 40GB
make staging-provision

# Provisionner prod avec 100GB
make prod-provision
```

### 4. Notes techniques

- Pour les instances **BASIC2-A4C-8G** (control-plane), Scaleway crée un volume local (`l_ssd`)
- Le redimensionnement d'un volume local nécessite l'**arrêt de l'instance**
- Le script gère automatiquement l'arrêt/redémarrage si un redimensionnement est nécessaire
- Si le volume est déjà >= taille cible, aucune action n'est effectuée

### 5. Vérification

Après provisioning, vérifiez la taille du disque sur la VM:

```bash
ssh user@staging-vm
df -h /
# Devrait afficher ~40GB pour staging, ~100GB pour prod
```

## Fichiers modifiés

- `scripts/scw_instance_provision.sh`: Ajout support `SCW_ROOT_VOLUME_SIZE_GB`
- `env/staging.env.example`: Ajout `SCW_ROOT_VOLUME_SIZE_GB=40`
- `env/prod.env.example`: Ajout `SCW_ROOT_VOLUME_SIZE_GB=100`

