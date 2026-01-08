# Configuration Multi-Worktree avec Isolation Complète

## Vue d'ensemble

Cette configuration permet d'exécuter plusieurs worktrees Git en parallèle sur la même machine, chacun avec son propre environnement isolé :
- ✅ Base de données PostgreSQL isolée (port et volume différents)
- ✅ UI sur des ports différents
- ✅ API sur des ports différents (via `make api-expose`)
- ✅ Aucun conflit de données ou de migrations

## Configuration

### 1. Ports et Volumes

Chaque worktree utilise :
- **UI** : `3000 + PORT_OFFSET`
- **API** : `8003 + PORT_OFFSET` (via `make api-expose`)
- **DB** : `5432 + PORT_OFFSET` (port host)
- **Volume DB** : `db_data_${PORT_OFFSET}`

### 2. Exemples d'utilisation

#### Worktree A (PORT_OFFSET=0) - Worktree principal
```bash
cd /path/to/worktree-a
PORT_OFFSET=0 make up
PORT_OFFSET=0 make ui
PORT_OFFSET=0 make api-expose

# Accès :
# - UI : http://localhost:3000
# - API : http://localhost:8003
# - DB : localhost:5432
# - Volume : db_data_0
```

#### Worktree B (PORT_OFFSET=10000) - Branche de développement
```bash
cd /path/to/worktree-b
PORT_OFFSET=10000 make up
PORT_OFFSET=10000 make ui
PORT_OFFSET=10000 make api-expose

# Accès :
# - UI : http://localhost:13000
# - API : http://localhost:18003
# - DB : localhost:15432
# - Volume : db_data_10000
```

#### Worktree C (PORT_OFFSET=20000) - Branche de test
```bash
cd /path/to/worktree-c
PORT_OFFSET=20000 make up
PORT_OFFSET=20000 make ui
PORT_OFFSET=20000 make api-expose

# Accès :
# - UI : http://localhost:23000
# - API : http://localhost:28003
# - DB : localhost:25432
# - Volume : db_data_20000
```

## Configuration dans env/dev.env

Dans chaque worktree, configurez `PORT_OFFSET` dans `env/dev.env` :

```bash
# Multi-worktree port offset
PORT_OFFSET=0        # Worktree A
# PORT_OFFSET=10000  # Worktree B
# PORT_OFFSET=20000  # Worktree C
```

Le Makefile calcule automatiquement :
- `UI_HOST_PORT = 3000 + PORT_OFFSET`
- `DB_HOST_PORT = 5432 + PORT_OFFSET`

## Volumes Docker

Les volumes sont créés automatiquement lors du premier démarrage :

```bash
# Lister les volumes DB
docker volume ls | grep db_data

# Supprimer un volume spécifique (attention : supprime toutes les données)
docker volume rm inventiv-agents_db_data_0
docker volume rm inventiv-agents_db_data_10000
```

## Connexion à la DB depuis l'extérieur

Pour vous connecter à la DB d'un worktree spécifique depuis un outil externe (psql, DBeaver, etc.) :

```bash
# Worktree A (PORT_OFFSET=0)
psql -h localhost -p 5432 -U postgres -d inventiv-agents

# Worktree B (PORT_OFFSET=10000)
psql -h localhost -p 15432 -U postgres -d inventiv-agents

# Worktree C (PORT_OFFSET=20000)
psql -h localhost -p 25432 -U postgres -d inventiv-agents
```

## Commandes Makefile

Toutes les commandes Makefile supportent `PORT_OFFSET` :

```bash
# Démarrer la stack
PORT_OFFSET=10000 make up

# Démarrer l'UI
PORT_OFFSET=10000 make ui

# Exposer l'API
PORT_OFFSET=10000 make api-expose

# Voir les logs
PORT_OFFSET=10000 make logs

# Arrêter la stack
PORT_OFFSET=10000 make down

# Voir les services
PORT_OFFSET=10000 make ps
```

## Avantages de l'isolation complète

1. **Pas de conflits de migrations** : Chaque worktree peut avoir des migrations différentes
2. **Données isolées** : Les données de test ne se mélangent pas
3. **Tests indépendants** : Chaque worktree peut tester sans affecter les autres
4. **Développement parallèle** : Plusieurs branches peuvent être testées simultanément

## Notes importantes

1. **Cloudflare Tunnel** : Si vous utilisez `WORKER_CONTROL_PLANE_URL` avec cloudflared, chaque worktree doit avoir son propre tunnel et sa propre URL
2. **Ressources** : Chaque worktree consomme des ressources (CPU, RAM, disque)
3. **Ports** : Assurez-vous que les ports calculés ne sont pas déjà utilisés
4. **Volumes** : Les volumes persistent même après `make down` (utilisez `make nuke` pour supprimer les volumes)

## Dépannage

### Port déjà utilisé

Si un port est déjà utilisé, choisissez un autre `PORT_OFFSET` :

```bash
# Vérifier les ports utilisés
lsof -i :5432
lsof -i :15432
lsof -i :3000
lsof -i :13000
```

### Volume corrompu

Si un volume est corrompu, supprimez-le et recréez-le :

```bash
# Arrêter le worktree
PORT_OFFSET=10000 make down

# Supprimer le volume
docker volume rm inventiv-agents_db_data_10000

# Redémarrer (le volume sera recréé)
PORT_OFFSET=10000 make up
```

### Conflit de réseau Docker

Si plusieurs worktrees créent des réseaux Docker avec le même nom, utilisez des noms de projet différents :

```bash
# Utiliser un nom de projet différent
COMPOSE_PROJECT_NAME=inventiv-worktree-b PORT_OFFSET=10000 make up
```
