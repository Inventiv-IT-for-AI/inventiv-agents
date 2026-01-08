# Enrichissement de l'Affichage des Volumes avec Historique Complet

**Date**: 2026-01-06  
**Objectif**: Ajouter une section détaillée listant tous les volumes (storages) d'une instance avec leur historique complet et statuts

---

## 📋 État Actuel

### API (`inventiv-api`)

**Endpoint**: `GET /instances/:id`

**Structure actuelle** (`InstanceStorageInfo`):
```rust
pub struct InstanceStorageInfo {
    pub provider_volume_id: String,
    pub name: Option<String>,
    pub volume_type: String,
    pub size_gb: Option<i64>,
    pub is_boot: bool,
}
```

**Limitations**:
- ❌ Seuls les volumes **non supprimés** (`deleted_at IS NULL`) sont retournés
- ❌ Pas d'historique (pas de `created_at`, `attached_at`, `deleted_at`, `reconciled_at`)
- ❌ Pas de statut détaillé (`status`, `delete_on_terminate`)
- ❌ Pas d'informations de réconciliation (`last_reconciliation`, `error_message`)

**Requête SQL actuelle**:
```sql
SELECT
  provider_volume_id,
  provider_volume_name,
  volume_type,
  size_bytes,
  is_boot
FROM instance_volumes
WHERE instance_id = $1 AND deleted_at IS NULL  -- ❌ Exclut les volumes supprimés
ORDER BY is_boot DESC, size_bytes DESC
```

### Frontend (`inventiv-frontend`)

**Affichage actuel** (`InstanceTimelineModal.tsx`):
- Affiche seulement le count et les tailles : `"2 storages (50GB, 200GB)"`
- Liste basique des volumes actifs avec : type, taille, nom, ID, flag boot
- ❌ Pas d'historique
- ❌ Pas de statuts détaillés
- ❌ Pas de volumes supprimés

---

## 🎯 Objectif

Créer une **section dédiée "Volumes History"** dans le modal d'instance qui affiche :

1. **Tous les volumes** (actifs ET supprimés) avec leur historique complet
2. **Statuts détaillés** : `attached`, `deleting`, `deleted`, `reconciled`
3. **Timestamps** : `created_at`, `attached_at`, `deleted_at`, `reconciled_at`
4. **Informations de réconciliation** : `last_reconciliation`, `error_message`
5. **Badges visuels** pour les statuts (actif, à supprimer, supprimé, réconcilié)

---

## 🔧 Modifications Requises

### 1. API - Enrichir `InstanceStorageInfo`

**Fichier**: `inventiv-api/src/main.rs`

**Changements**:
```rust
#[derive(Serialize, utoipa::ToSchema)]
pub struct InstanceStorageInfo {
    // Identifiants
    pub id: uuid::Uuid,
    pub provider_volume_id: String,
    pub name: Option<String>,
    pub volume_type: String,
    pub size_gb: Option<i64>,
    pub is_boot: bool,
    
    // Statut et cycle de vie
    pub status: String,  // 'attached', 'detached', 'deleting', 'deleted'
    pub delete_on_terminate: bool,
    
    // Timestamps (historique complet)
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub attached_at: Option<chrono::DateTime<chrono::Utc>>,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub reconciled_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_reconciliation: Option<chrono::DateTime<chrono::Utc>>,
    
    // Erreurs et réconciliation
    pub error_message: Option<String>,
}
```

**Modification de la requête SQL**:
```sql
SELECT
  iv.id,
  iv.provider_volume_id,
  iv.provider_volume_name,
  iv.volume_type,
  iv.size_bytes,
  iv.is_boot,
  iv.status,
  iv.delete_on_terminate,
  iv.created_at,
  iv.attached_at,
  iv.deleted_at,
  iv.reconciled_at,
  iv.last_reconciliation,
  iv.error_message
FROM instance_volumes iv
WHERE iv.instance_id = $1  -- ✅ Retourne TOUS les volumes (même supprimés)
ORDER BY 
  -- Actifs en premier, puis par date de création décroissante
  CASE WHEN iv.deleted_at IS NULL THEN 0 ELSE 1 END,
  iv.created_at DESC
```

### 2. Frontend - Types TypeScript

**Fichier**: `inventiv-frontend/src/lib/types.ts`

**Changements**:
```typescript
export type InstanceStorageInfo = {
    id: string;
    provider_volume_id: string;
    name?: string | null;
    volume_type: string;
    size_gb?: number | null;
    is_boot: boolean;
    
    // Statut et cycle de vie
    status: string;  // 'attached', 'detached', 'deleting', 'deleted'
    delete_on_terminate: boolean;
    
    // Timestamps (historique complet)
    created_at: string;
    attached_at?: string | null;
    deleted_at?: string | null;
    reconciled_at?: string | null;
    last_reconciliation?: string | null;
    
    // Erreurs et réconciliation
    error_message?: string | null;
};
```

### 3. Frontend - Nouveau Composant `InstanceVolumesHistory`

**Fichier**: `inventiv-frontend/src/components/instances/InstanceVolumesHistory.tsx` (nouveau)

**Fonctionnalités**:
- Table ou liste détaillée de tous les volumes
- Badges de statut avec couleurs :
  - `attached` → vert (actif)
  - `deleting` → orange (en cours de suppression)
  - `deleted` → gris (supprimé, en attente de réconciliation)
  - `reconciled` → gris foncé (réconcilié, confirmé supprimé)
- Colonnes : ID, Type, Taille, Statut, Dates (création, attachement, suppression, réconciliation)
- Filtres : Actifs uniquement / Tous / Supprimés uniquement
- Tri : Par date de création, statut, taille

### 4. Frontend - Intégration dans `InstanceTimelineModal`

**Fichier**: `inventiv-frontend/src/components/instances/InstanceTimelineModal.tsx`

**Changements**:
- Remplacer la section Storage actuelle (lignes 554-592) par le nouveau composant `InstanceVolumesHistory`
- Ajouter un onglet ou section dédiée "Volumes History"
- Afficher le résumé (count actifs) dans la vue principale
- Afficher l'historique complet dans une section expandable ou un onglet séparé

---

## 📊 Structure de Données Proposée

### Exemple de Réponse API Enrichie

```json
{
  "instance": { ... },
  "storages": [
    {
      "id": "uuid-1",
      "provider_volume_id": "4a7faac7-16ad-4861-9352-e1a9b617fe5b",
      "name": "boot-volume-l4-1",
      "volume_type": "sbs_volume",
      "size_gb": 50,
      "is_boot": true,
      "status": "deleted",
      "delete_on_terminate": true,
      "created_at": "2026-01-05T21:05:40Z",
      "attached_at": "2026-01-05T21:05:45Z",
      "deleted_at": "2026-01-06T08:46:51Z",
      "reconciled_at": null,
      "last_reconciliation": "2026-01-06T08:47:00Z",
      "error_message": null
    },
    {
      "id": "uuid-2",
      "provider_volume_id": "data-volume-123",
      "name": "data-volume-200gb",
      "volume_type": "sbs_volume",
      "size_gb": 200,
      "is_boot": false,
      "status": "attached",
      "delete_on_terminate": true,
      "created_at": "2026-01-05T21:06:00Z",
      "attached_at": "2026-01-05T21:06:05Z",
      "deleted_at": null,
      "reconciled_at": null,
      "last_reconciliation": null,
      "error_message": null
    }
  ]
}
```

---

## 🎨 Design UI Proposé

### Section "Volumes History" dans le Modal

```
┌─────────────────────────────────────────────────────────────┐
│ Volumes History                                             │
├─────────────────────────────────────────────────────────────┤
│ Filters: [All] [Active] [Deleted] [Reconciled]              │
│                                                             │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ Volume 1: boot-volume-l4-1                             │ │
│ │ Type: sbs_volume | Size: 50GB | Boot: Yes                │ │
│ │ Status: [Deleted] 🟠                                     │ │
│ │ Created: 2026-01-05 21:05:40                            │ │
│ │ Attached: 2026-01-05 21:05:45                           │ │
│ │ Deleted: 2026-01-06 08:46:51                            │ │
│ │ Last Reconciliation: 2026-01-06 08:47:00                │ │
│ │ Reconciled: Pending ⏳                                   │ │
│ └─────────────────────────────────────────────────────────┘ │
│                                                             │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ Volume 2: data-volume-200gb                             │ │
│ │ Type: sbs_volume | Size: 200GB | Boot: No                │ │
│ │ Status: [Attached] 🟢                                    │ │
│ │ Created: 2026-01-05 21:06:00                            │ │
│ │ Attached: 2026-01-05 21:06:05                           │ │
│ └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

### Badges de Statut

- 🟢 **Attached** : Volume actif et attaché
- 🟠 **Deleting** : En cours de suppression
- ⚫ **Deleted** : Supprimé, en attente de réconciliation
- ⚪ **Reconciled** : Réconcilié, confirmé supprimé chez le provider

---

## 📝 Plan d'Implémentation

### Phase 1 : API (Backend)

1. ✅ Enrichir `InstanceStorageInfo` avec tous les champs nécessaires
2. ✅ Modifier la requête SQL pour retourner TOUS les volumes (même supprimés)
3. ✅ Tester l'endpoint `GET /instances/:id` avec des volumes supprimés

### Phase 2 : Types Frontend

1. ✅ Mettre à jour `InstanceStorageInfo` dans `types.ts`
2. ✅ Vérifier la compatibilité avec les composants existants

### Phase 3 : Composant Frontend

1. ✅ Créer `InstanceVolumesHistory.tsx`
2. ✅ Implémenter l'affichage avec badges de statut
3. ✅ Ajouter filtres et tri
4. ✅ Intégrer dans `InstanceTimelineModal`

### Phase 4 : Tests & Validation

1. ✅ Tester avec instances ayant des volumes actifs
2. ✅ Tester avec instances ayant des volumes supprimés
3. ✅ Tester avec instances ayant des volumes en cours de réconciliation
4. ✅ Vérifier l'affichage responsive

---

## 🔍 Points d'Attention

### Performance
- La requête retourne maintenant TOUS les volumes (même supprimés)
- Pour les instances avec beaucoup d'historique, considérer la pagination si nécessaire
- L'index `idx_instance_volumes_reconciliation` devrait aider pour les requêtes

### Compatibilité
- Les composants existants qui utilisent `storages` doivent être compatibles avec les nouveaux champs
- Les champs optionnels (`attached_at`, `deleted_at`, etc.) doivent être gérés correctement

### UX
- L'affichage doit être clair et ne pas surcharger l'interface
- Les filtres permettent de réduire la complexité visuelle
- Les badges de statut doivent être intuitifs

---

## ✅ Checklist de Validation

- [ ] API retourne tous les volumes avec historique complet
- [ ] Types TypeScript mis à jour
- [ ] Composant `InstanceVolumesHistory` créé et fonctionnel
- [ ] Intégration dans `InstanceTimelineModal` réussie
- [ ] Badges de statut affichés correctement
- [ ] Filtres fonctionnent (All/Active/Deleted/Reconciled)
- [ ] Tri par date/statut fonctionne
- [ ] Tests avec instances réelles (actives, supprimées, en réconciliation)
- [ ] Documentation mise à jour

---

## 📚 Références

- [Storage Management](STORAGE_MANAGEMENT.md) - Documentation sur la gestion des volumes
- [Worker Reliability Analysis](WORKER_RELIABILITY_ANALYSIS.md) - Analyse de fiabilité
- [API Endpoints](ENDPOINTS_INVENTORY.md) - Inventaire des endpoints

