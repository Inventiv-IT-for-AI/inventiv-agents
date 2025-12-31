# Plan de Consolidation des Migrations

## Contexte

Avant la mise en production, il est recommandé de consolider les migrations pour :
- **Simplicité** : Schéma clair et à jour, facile à comprendre
- **Maintenabilité** : Moins de fichiers à gérer, moins de risques d'erreurs
- **Performance** : Moins de migrations à exécuter au démarrage
- **Clarté** : Schéma final visible d'un coup d'œil

## État Actuel

- **30 migrations** dans `sqlx-migrations/`
- **Baseline** : `00000000000000_baseline.sql` (38KB) - dump complet
- **Archive** : `sqlx-migrations-archive/` avec anciennes migrations

## Approche Recommandée

### Option A : Consolidation Complète (Recommandée avant prod)

1. **Créer une nouvelle baseline consolidée**
   - Générer le schéma actuel complet depuis la DB
   - Nettoyer et organiser le schéma de manière logique
   - Inclure toutes les tables, types, fonctions, index, etc.

2. **Archiver les anciennes migrations**
   - Déplacer toutes les migrations actuelles vers `sqlx-migrations-archive/`
   - Garder seulement la nouvelle baseline

3. **Nettoyer les seeds**
   - S'assurer que `seeds/catalog_seeds.sql` est cohérent avec le nouveau schéma
   - Supprimer les données obsolètes ou redondantes

### Option B : Consolidation Partielle (Compromis)

1. **Garder la baseline actuelle**
2. **Consolider les migrations récentes** (derniers mois)
3. **Archiver les anciennes migrations** (avant une date donnée)

## Plan d'Action

### Étape 1 : Backup
```bash
# Backup complet de la DB actuelle
docker compose exec db pg_dump -U postgres llminfra > backup_before_consolidation.sql

# Backup des migrations actuelles
cp -r sqlx-migrations sqlx-migrations-backup-$(date +%Y%m%d)
```

### Étape 2 : Générer le schéma consolidé
```bash
# Générer le schéma complet depuis la DB
docker compose exec db pg_dump -U postgres llminfra --schema-only > sqlx-migrations/00000000000000_baseline_consolidated.sql

# Nettoyer le fichier (supprimer les commentaires pg_dump, garder seulement le SQL)
```

### Étape 3 : Archiver les anciennes migrations
```bash
# Déplacer toutes les migrations vers archive
mkdir -p sqlx-migrations-archive/consolidated-$(date +%Y%m%d)
mv sqlx-migrations/202*.sql sqlx-migrations-archive/consolidated-$(date +%Y%m%d)/
```

### Étape 4 : Tester
```bash
# Tester avec une DB vierge
docker compose down -v
docker compose up -d db
# Vérifier que les migrations s'appliquent correctement
```

### Étape 5 : Nettoyer les seeds
- Vérifier que `seeds/catalog_seeds.sql` est à jour
- Supprimer les références aux anciennes structures

## Avantages

✅ **Simplicité** : Un seul fichier baseline au lieu de 30 migrations
✅ **Clarté** : Schéma complet visible immédiatement
✅ **Performance** : Une seule migration à exécuter
✅ **Maintenabilité** : Plus facile à comprendre et modifier
✅ **Historique préservé** : Migrations archivées pour référence

## Inconvénients

⚠️ **Perte de l'historique détaillé** : Impossible de voir l'évolution étape par étape
⚠️ **Migration unique** : Plus difficile de rollback une partie spécifique
⚠️ **Risque** : Si la consolidation est mal faite, peut casser les déploiements

## Recommandation

**Avant production** : Option A (Consolidation complète)
- Vous n'avez pas encore de données critiques en prod
- C'est le moment idéal pour nettoyer
- Facilite la maintenance future

**Après production** : Garder l'historique des migrations
- Chaque changement devient une migration incrémentale
- Traçabilité importante pour la production

## Notes

- Les migrations SQLx sont trackées dans `_sqlx_migrations`
- Après consolidation, cette table sera réinitialisée
- Les seeds doivent être idempotents (ON CONFLICT DO NOTHING/UPDATE)

