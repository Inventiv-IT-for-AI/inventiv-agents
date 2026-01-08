# Configuration de la Base de Données pour Multi-Worktree

## Problématique

Quand vous utilisez plusieurs worktrees Git en parallèle pour tester différentes versions/branches, vous avez deux options pour la base de données :

1. **Isolation complète** : Chaque worktree a sa propre instance DB (recommandé)
2. **Partage de DB** : Tous les worktrees partagent la même DB (plus simple mais risques de conflits)

## Option 1 : Isolation Complète (Recommandé)

### Avantages
- ✅ Aucun conflit de données entre worktrees
- ✅ Chaque worktree peut avoir des migrations différentes
- ✅ Tests indépendants
- ✅ Pas de risque de corruption de données

### Configuration

Pour isoler complètement chaque worktree, vous devez :

1. **Utiliser PORT_OFFSET pour le port de la DB** dans `docker-compose.yml`
2. **Utiliser des volumes Docker différents** pour chaque worktree
3. **Utiliser des noms de DB différents** (optionnel mais recommandé)

#### Modification de `docker-compose.yml`

```yaml
db:
  image: timescale/timescaledb:latest-pg14
  environment:
    - POSTGRES_USER=postgres
    - POSTGRES_PASSWORD=password
    - POSTGRES_DB=${POSTGRES_DB:-inventiv-agents}
  volumes:
    - db_data_${PORT_OFFSET:-0}:/var/lib/postgresql/data
  ports:
    - "${DB_HOST_PORT:-5432}:5432"
```

#### Modification de `env/dev.env`

```bash
# DB (dev) - isolée par worktree via PORT_OFFSET
POSTGRES_USER=postgres
POSTGRES_PASSWORD=password
POSTGRES_DB=inventiv-agents-${PORT_OFFSET:-0}  # ou simplement inventiv-agents si vous utilisez des volumes différents
DB_HOST_PORT=$((5432 + ${PORT_OFFSET:-0}))  # Worktree A: 5432, Worktree B: 15432, etc.
```

#### Mise à jour des DATABASE_URL

Dans `docker-compose.yml`, utilisez une variable d'environnement :

```yaml
api:
  environment:
    - DATABASE_URL=postgresql://postgres:password@db:5432/${POSTGRES_DB:-inventiv-agents}
```

#### Mise à jour des volumes

```yaml
volumes:
  redis_data:
  db_data_0:      # Worktree avec PORT_OFFSET=0
  db_data_10000:  # Worktree avec PORT_OFFSET=10000
  ui_node_modules:
```

### Exemple d'utilisation

**Worktree A** (PORT_OFFSET=0) :
```bash
PORT_OFFSET=0 make up
# DB sur localhost:5432
# Volume: db_data_0
# DB name: inventiv-agents-0
```

**Worktree B** (PORT_OFFSET=10000) :
```bash
PORT_OFFSET=10000 make up
# DB sur localhost:15432
# Volume: db_data_10000
# DB name: inventiv-agents-10000
```

## Option 2 : Partage de DB (Plus Simple)

### Avantages
- ✅ Configuration plus simple
- ✅ Moins de ressources utilisées
- ✅ Facile à mettre en place

### Inconvénients
- ⚠️ Risque de conflits de données entre worktrees
- ⚠️ Migrations peuvent entrer en conflit
- ⚠️ Tests peuvent interférer entre eux

### Configuration

Avec cette option, **vous n'avez PAS besoin de PORT_OFFSET pour la DB** :

1. **Un seul port** : `5432:5432` (fixe)
2. **Un seul volume** : `db_data` (partagé)
3. **Un seul nom de DB** : `inventiv-agents` (partagé)

Chaque worktree se connecte à la même instance DB, mais peut utiliser des **schémas différents** ou simplement accepter les risques de conflits.

### Exemple avec schémas différents

```sql
-- Worktree A utilise le schéma par défaut (public)
-- Worktree B utilise un schéma dédié
CREATE SCHEMA IF NOT EXISTS worktree_b;
SET search_path TO worktree_b;
```

Mais cela nécessite des modifications dans le code pour gérer les schémas.

## Recommandation

Pour le développement local avec plusieurs worktrees, **l'Option 1 (isolation complète) est recommandée** car :

1. Elle évite tous les conflits
2. Elle permet de tester différentes migrations
3. Elle est plus sûre pour le développement

## Implémentation Recommandée

### 1. Modifier `docker-compose.yml`

```yaml
db:
  image: timescale/timescaledb:latest-pg14
  environment:
    - POSTGRES_USER=postgres
    - POSTGRES_PASSWORD=password
    - POSTGRES_DB=${POSTGRES_DB:-inventiv-agents}
  volumes:
    - db_data_${PORT_OFFSET:-0}:/var/lib/postgresql/data
  ports:
    - "${DB_HOST_PORT:-5432}:5432"

# ... dans les services api/orchestrator/finops
  environment:
    - DATABASE_URL=postgresql://postgres:password@db:5432/${POSTGRES_DB:-inventiv-agents}

volumes:
  redis_data:
  db_data_0:
  db_data_10000:
  db_data_20000:
  ui_node_modules:
```

### 2. Modifier `env/dev.env`

```bash
# DB (dev) - isolée par worktree
POSTGRES_USER=postgres
POSTGRES_PASSWORD=password
POSTGRES_DB=inventiv-agents
# DB_HOST_PORT sera calculé automatiquement : 5432 + PORT_OFFSET
```

### 3. Modifier le Makefile (optionnel)

Ajouter une variable pour calculer le port de la DB :

```makefile
DB_HOST_PORT ?= $(shell off="$(PORT_OFFSET)"; if [ -z "$$off" ]; then off=0; fi; echo $$((5432 + $$off)))
```

Puis l'utiliser dans les commandes docker-compose.

## Notes Importantes

1. **Volumes Docker** : Chaque worktree avec un PORT_OFFSET différent aura son propre volume, donc ses propres données
2. **Ports host** : Si vous exposez la DB sur le host (pour psql, DBeaver, etc.), utilisez le port calculé avec PORT_OFFSET
3. **Connexions externes** : Les outils externes (psql, DBeaver) doivent utiliser le port host calculé, pas le port interne du conteneur (toujours 5432)

## Exemple Complet

**Worktree A** (`PORT_OFFSET=0`) :
- UI : `http://localhost:3000`
- API : `http://localhost:8003` (via `make api-expose`)
- DB host : `localhost:5432`
- Volume : `db_data_0`
- DB name : `inventiv-agents`

**Worktree B** (`PORT_OFFSET=10000`) :
- UI : `http://localhost:13000`
- API : `http://localhost:18003` (via `PORT_OFFSET=10000 make api-expose`)
- DB host : `localhost:15432`
- Volume : `db_data_10000`
- DB name : `inventiv-agents`
