# Implémentation du système d'association Instance Type ↔ Zones

## ✅ Ce qui a été implémenté

### 1. **Migration DB** ✅
```sql
CREATE TABLE instance_type_zones (
    instance_type_id UUID REFERENCES instance_types(id) ON DELETE CASCADE,
    zone_id UUID REFERENCES zones(id) ON DELETE CASCADE,
    is_available BOOLEAN DEFAULT true,
    created_at TIMESTAMP DEFAULT NOW(),
    PRIMARY KEY (instance_type_id, zone_id)
);
```

**Données de test** : RENDER-S associé à Paris 1, Paris 2, Amsterdam 1

### 2. **Backend API** ✅

**Fichier** : `/inventiv-api/src/instance_type_zones.rs`

**Endpoints créés** :
| Méthode | Path | Description |
|---------|------|-------------|
| GET | `/instance_types/{id}/zones` | Liste les zones associées à un type d'instance |
| PUT | `/instance_types/{id}/zones` | Associe/Dissocie des zones pour un type |
| GET | `/zones/{zone_id}/instance_types` | Liste les types disponibles dans une zone |

**Test** :
```bash
curl http://localhost:8003/instance_types/00000000-0000-0000-0000-000000000030/zones
# Retourne: Amsterdam 1, Paris 1, Paris 2
```

## 🔨 Ce qui reste à faire

### 3. **Frontend - Settings UI** 🚧

**Fichier à modifier** : `/inventiv-frontend/src/app/settings/page.tsx`

**Fonctionnalités à ajouter** :
1. Bouton "Manage Zones" sur chaque ligne d'Instance Type
2. Dialog/Modal pour :
   - Afficher les zones disponibles
   - Checkbox pour sélectionner/désélectionner
   - Bouton "Save" qui appelle `PUT /instance_types/{id}/zones`
3. Indicateur visuel du nombre de zones associées (ex: badge "3 zones")

**Exemple d'UI** :
```
Instance Types Table
┌─────────────────────────────────────────────────────────┐
│ Name     │ GPU │ VRAM │ Price  │ Zones     │ Actions   │
├─────────────────────────────────────────────────────────┤
│ RENDER-S │ 1   │ 22GB │ $0.50  │ 🌍 3 zones│ [Manage]  │
└─────────────────────────────────────────────────────────┘

Clicking [Manage] opens:
┌────────────────────────────────────┐
│ Manage Zones for RENDER-S         │
├────────────────────────────────────┤
│ ☑ Paris 1 (fr-par-1)              │
│ ☑ Paris 2 (fr-par-2)              │
│ ☑ Amsterdam 1 (nl-ams-1)          │
│ ☐ London 1 (uk-lon-1)             │
├────────────────────────────────────┤
│        [Cancel]  [Save Changes]    │
└────────────────────────────────────┘
```

### 4. **Frontend - Dashboard Filtering** 🚧

**Fichier à modifier** : `/inventiv-frontend/src/app/page.tsx`

**Changement minimal** (ligne 171-173) :

```typescript
// ❌ AVANT
const availableTypes = selectedProviderId
  ? instanceTypes.filter(t => t.provider_id === selectedProviderId)
  : [];

// ✅ APRÈS
const availableTypes = useMemo(() => {
  if (!selectedProviderId) return [];
  
  let filtered = instanceTypes.filter(t => t.provider_id === selectedProviderId);
  
  // Si une zone est sélectionnée, appeler l'endpoint de filtrage
  if (selectedZoneId && filtered.length > 0) {
    // Option 1: Charger via API (recommandé)
    fetch(`/api/backend/zones/${selectedZoneId}/instance_types`)
      .then(res => res.json())
      .then(setInstanceTypes);
    
    // Option 2: Filtrer en frontend (fallback)
    // filtered = filtered.filter(type => 
    //   type.available_zones?.includes(selectedZoneCode)
    // );
  }
  
  return filtered;
}, [selectedProviderId, selectedZoneId]);
```

### 5. **Tests** 🚧

**Scénarios à tester** :
1. ✅ Créer associations (DB migration)  
2. ✅ GET /instance_types/{id}/zones → retourne zones
3. ⏹️ PUT /instance_types/{id}/zones → modifie associations
4. ⏹️ Dashboard : sélectionner zone → voir types filtrés
5. ⏹️ Settings : gérer zones via UI

## 🎯 Plan d'action pour terminer

### Étape 1 : Settings UI (30-45 min)
1. Ajouter colonne "Zones" dans le tableau
2. Créer `ManageZonesDialog` component
3. Charger zones depuis `/zones`
4. Charger associations depuis `/instance_types/{id}/zones`
5. Sauvegarder modifications via PUT

### Étape 2 : Dashboard Filtering (15 min)
1. Modifier `availableTypes` pour utiliser `/zones/{id}/instance_types`
2. Ajouter `useEffect` qui se déclenche quand `selectedZoneId` change
3. Tester le flow complet

### Étape 3 : Polish & Tests (15 min)
1. Loading states
2. Error handling
3. Success notifications
4. Tests end-to-end

## 📊 Bénéfices

✅ **Utilisateur** :
- Ne voit que les types d'instance réellement disponibles
- Pas d'erreur "Type not available in zone"
- Meilleure UX

✅ **Admin** :
- Gestion facile des disponibilités via Settings
- Pas besoin de modifier la DB manuellement
- Flexibilité pour ajouter/retirer zones

✅ **Système** :
- Architecture propre et extensible
- API REST complète
- Données normalisées
