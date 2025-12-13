# Format d'affichage des actions - Documentation

## Format standardisé

### Backend (Base de données)
Format: **`UPPER_CASE_WITH_UNDERSCORE`**
- Exemple: `PROVIDER_CREATE`, `EXECUTE_TERMINATE`, `INSTANCE_CREATED`

### Frontend (Affichage utilisateur)
Format: **`Title Case With Spaces`**
- Transformation automatique: `action_type.replace(/_/g, ' ').replace(/\b\w/g, l => l.toUpperCase())`
- Exemples:
  - `PROVIDER_CREATE` → "Provider Create"
  - `EXECUTE_TERMINATE` → "Execute Terminate"
  - `INSTANCE_CREATED` → "Instance Created"
  - `ARCHIVE_INSTANCE` → "Archive Instance"

## Actions du cycle de vie complet

### 🔵 Création d'instance
| Action Backend | Label Frontend | Icône | Component |
|----------------|----------------|-------|-----------|
| `REQUEST_CREATE` | Request Create | ⚡ Zap | API |
| `EXECUTE_CREATE` | Execute Create | 🖥️ Server | Orchestrator |
| `PROVIDER_CREATE` | Provider Create | ☁️ Cloud | Orchestrator |
| `INSTANCE_CREATED` | Instance Created | 🗄️ Database | Orchestrator |

### 🔴 Terminaison d'instance
| Action Backend | Label Frontend | Icône | Component |
|----------------|----------------|-------|-----------|
| `REQUEST_TERMINATE` | Request Terminate | ⚡ Zap | API |
| `EXECUTE_TERMINATE` | Execute Terminate | 🖥️ Server | Orchestrator |
| `PROVIDER_TERMINATE` | Provider Terminate | ☁️ Cloud | Orchestrator |
| `INSTANCE_TERMINATED` | Instance Terminated | 🗄️ Database | Orchestrator |

### 📦 Archivage
| Action Backend | Label Frontend | Icône | Component |
|----------------|----------------|-------|-----------|
| `ARCHIVE_INSTANCE` | Archive Instance | 📦 Archive | API |

### 🔍 Réconciliation & monitoring
| Action Backend | Label Frontend | Icône | Component |
|----------------|----------------|-------|-----------|
| `PROVIDER_DELETED_DETECTED` | Provider Deleted Detected | ⚠️ AlertTriangle | Orchestrator |

## Actions legacy (à supprimer progressivement)
| Action Backend | Remplacé par | Notes |
|----------------|--------------|-------|
| `SCALEWAY_CREATE` | `PROVIDER_CREATE` | Nom spécifique au provider, non générique |
| `SCALEWAY_DELETE` | `PROVIDER_TERMINATE` | Nom spécifique au provider, non générique |
| `TERMINATE_INSTANCE` | `EXECUTE_TERMINATE` | Nomenclature incorrecte |

## Couleurs & styles

### Monitoring Page (Badges)
- **Request** : Bleu (`bg-blue-500/600`)
- **Execute** : Violet (`bg-purple-500/600`)
- **Provider** : Orange (`bg-orange-500/600`)
- **Instance** : Vert (création) / Rouge (terminaison)
- **Archive** : Gris
- **Détection d'anomalies** : Jaune

### Timeline Modal (Cards)
- Bordure gauche colorée selon l'action
- Background léger assorti
- Icônes rondes avec la même couleur

## Règles d'uniformité

✅ **À faire**
- Toujours utiliser `UPPER_CASE_WITH_UNDERSCORE` dans le backend
- Toujours convertir en `Title Case With Spaces` dans le frontend
- Utiliser des noms génériques (pas de nom de provider)
- Maintenir la symétrie création/terminaison

❌ **À éviter**
- Mélanger les formats (ex: `createInstance` ou `Create_Instance`)
- Hard-coder des noms de providers (ex: `SCALEWAY_*`)
- Utiliser des abréviations non évidentes
- Avoir des actions sans log correspondant
