# Amélioration du filtrage des Instance Types

## 🔍 Problème identifié

Lors de la création d'une instance, la liste des types d'instance (Instance Types) affiche **tous les types du provider sélectionné**, même ceux qui ne sont pas disponibles dans la zone choisie.

### Flux actuel

```
Frontend
  ├─ Charge TOUS les types via /instance_types
  └─ Filtre par provider_id uniquement (ligne 171-173)
      ❌ Pas de filtre par zone/région

Backend (/instance_types)
  └─ SELECT * FROM instance_types WHERE is_active = true
      ❌ Aucun filtre zone/région

DB (instance_types)
  └─ Colonnes: id, provider_id, name, code, gpu_count, ...
      ❌ Pas de relation avec zones
```

## 💡 Solutions proposées

### Option A : Table de liaison (Complex)

**Avantages:**
- Modèle de données normalisé
- Facile de gérer les changements de disponibilité

**Inconvénients:**
- Nécessite migration DB
- Plus de JOINs dans les requêtes
- Maintenance de la table de liaison

```sql
CREATE TABLE zone_instance_types (
    zone_id UUID REFERENCES zones(id),
    instance_type_id UUID REFERENCES instance_types(id),
    is_available BOOLEAN DEFAULT true,
    PRIMARY KEY (zone_id, instance_type_id)
);
```

### Option B : Metadata JSON (Simple) ⭐ **RECOMMANDÉ**

**Avantages:**
- Pas de changement de schéma majeur
- Flexible pour ajouter d'autres metadata
- Facile à implémenter

**Inconvénients:**
- Moins normalisé
- Requêtes JSON un peu plus complexes

```sql
ALTER TABLE instance_types 
ADD COLUMN metadata JSONB DEFAULT '{}';

-- Exemple de données
UPDATE instance_types 
SET metadata = '{"available_zones": ["fr-par-1", "fr-par-2"]}'
WHERE code = 'H100-1-80GB';
```

### Option C : Filtre dynamique via API provider (Plus complexe)

Interroger l'API du provider en temps réel pour les types disponibles.

**Avantages:**
- Toujours à jour
- Pas besoin de maintenir les données

**Inconvénients:**
- Lent (requête API à chaque chargement)
- Dépendance externe
- Coût API

## 🎯 Implémentation recommandée : Option B

### Étape 1 : Migration DB

```sql
-- Ajouter la colonne metadata
ALTER TABLE instance_types 
ADD COLUMN metadata JSONB DEFAULT '{}';

-- Exemple: Configurer les zones pour Scaleway RENDER-S
UPDATE instance_types 
SET metadata = jsonb_build_object(
    'available_zones', ARRAY['fr-par-1', 'fr-par-2', 'nl-ams-1']
)
WHERE provider_id = (SELECT id FROM providers WHERE name = 'Scaleway')
  AND code = 'RENDER-S';
```

### Étape 2 : Backend - Endpoint amélioré

```rust
// Option 1: Nouveau endpoint avec filtres
#[utoipa::path(
    get,
    path = "/instance_types/available",
    params(
        ("zone_id" = Option<Uuid>, Query, description = "Filter by zone"),
        ("region_id" = Option<Uuid>, Query, description = "Filter by region")
    ),
    responses(
        (status = 200, body = Vec<InstanceType>)
    )
)]
pub async fn list_available_instance_types(
    State(state): State<Arc<AppState>>,
    Query(params): Query<AvailabilityFilter>,
) -> Json<Vec<InstanceType>> {
    // Si zone_id fourni, filtrer par metadata
    let types = if let Some(zone_id) = params.zone_id {
        let zone_code = get_zone_code(&state.db, zone_id).await;
        
        sqlx::query_as::<_, InstanceType>(
            r#"SELECT * FROM instance_types 
               WHERE is_active = true
               AND (metadata->>'available_zones' IS NULL 
                    OR metadata->'available_zones' @> $1::jsonb)
               ORDER BY name"#
        )
        .bind(json!([zone_code]))
        .fetch_all(&state.db)
        .await
        .unwrap_or(vec![])
    } else {
        // Si pas de filtre, retourner tous
        sqlx::query_as::<_, InstanceType>(
            "SELECT * FROM instance_types WHERE is_active = true ORDER BY name"
        )
        .fetch_all(&state.db)
        .await
        .unwrap_or(vec![])
    };
    
    Json(types)
}
```

### Étape 3 : Frontend - Filtrage amélioré

```typescript
// Dans page.tsx

// Option 1: Filtrer côté frontend avec metadata
const availableTypes = useMemo(() => {
  if (!selectedProviderId) return [];
  
  let filtered = instanceTypes.filter(t => t.provider_id === selectedProviderId);
  
  // Si une zone est sélectionnée, filtrer par disponibilité
  if (selectedZoneId) {
    const selectedZone = zones.find(z => z.id === selectedZoneId);
    filtered = filtered.filter(type => {
      // Si pas de metadata.available_zones, considérer comme disponible partout
      if (!type.metadata?.available_zones) return true;
      // Sinon vérifier si la zone est dans la liste
      return type.metadata.available_zones.includes(selectedZone?.code);
    });
  }
  
  return filtered;
}, [selectedProviderId, selectedZoneId, instanceTypes, zones]);

// Option 2: Appeler un endpoint dédié (meilleure performance)
useEffect(() => {
  if (selectedZoneId) {
    fetch(`/api/backend/instance_types/available?zone_id=${selectedZoneId}`)
      .then(res => res.json())
      .then(setInstanceTypes);
  }
}, [selectedZoneId]);
```

## 📋 Plan d'action proposé

1. **Court terme (rapide)** :
   - ✅ Ajouter colonne `metadata JSONB` à `instance_types`
   - ✅ Configurer les zones disponibles pour les types existants
   - ✅ Modifier le frontend pour filtrer par zone sélectionnée

2. **Moyen terme**:
   - Créer endpoint `/instance_types/available?zone_id=X`
   - Ajouter UI dans Settings pour gérer les zones disponibles par type

3. **Long terme**:
   - Sync automatique avec API provider pour vérifier disponibilité
   - Cache des résultats

## 🚀 Bénéfices

- ✅ Utilisateur voit seulement les types réellement disponibles
- ✅ Évite les erreurs "Type not available in this zone"
- ✅ Meilleure UX
- ✅ Architecture flexible pour futures améliorations
