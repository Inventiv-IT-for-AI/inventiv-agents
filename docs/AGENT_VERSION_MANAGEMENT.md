# Agent Version Management & Integrity

## Vue d'ensemble

Le système garantit l'intégrité et la traçabilité de `agent.py` via :
- **Versioning** : Numéro de version et date de build
- **Checksum SHA256** : Vérification d'intégrité
- **Monitoring** : Détection automatique des problèmes
- **CI/CD** : Automatisation de la mise à jour de version

## Version dans agent.py

### Constantes de version

Le fichier `inventiv-worker/agent.py` contient :

```python
# Version information (updated on each release)
AGENT_VERSION = "1.0.0"  # Update this when making changes to agent.py
AGENT_BUILD_DATE = "2026-01-03"  # Update this when making changes
```

### Calcul de checksum

Fonction automatique pour calculer le SHA256 :

```python
def _get_agent_checksum():
    """Calculate SHA256 checksum of this agent.py file."""
    # Calcule automatiquement le checksum du fichier
```

## Endpoints Worker

### `/info` (nouveau)

Endpoint HTTP exposant les informations de l'agent :

```bash
curl http://<worker-ip>:8080/info
```

**Réponse** :
```json
{
  "agent_version": "1.0.0",
  "agent_build_date": "2026-01-03",
  "agent_checksum": "4f9441dc6c913ecb9fda38fd1f9f7413c97df05af3d5fe8c11dd05b312be9ada",
  "agent_path": "/opt/inventiv-worker/agent.py",
  "python_version": "3.11.0",
  "worker_id": "uuid",
  "instance_id": "uuid",
  "model_id": "Qwen/Qwen2.5-7B-Instruct",
  "vllm_base_url": "http://127.0.0.1:8000",
  "vllm_ready": true
}
```

### Heartbeat enrichi

Les heartbeats incluent maintenant `agent_info` :

```json
{
  "instance_id": "uuid",
  "worker_id": "uuid",
  "status": "ready",
  "agent_info": {
    "version": "1.0.0",
    "build_date": "2026-01-03",
    "checksum": "4f9441dc..."
  },
  ...
}
```

## Vérification d'intégrité

### Script SSH Bootstrap

Le script de bootstrap vérifie automatiquement le checksum :

```bash
# Si WORKER_AGENT_SHA256 est défini
if [[ -n "$AGENT_EXPECTED_SHA256" ]]; then
  ACTUAL_SHA256=$(sha256sum /opt/inventiv-worker/agent.py | cut -d' ' -f1)
  if [[ "$ACTUAL_SHA256" != "$AGENT_EXPECTED_SHA256" ]]; then
    echo "ERROR: agent.py checksum mismatch!"
    exit 1
  fi
fi
```

### Health Check

Le health check vérifie automatiquement :
- Accessibilité de `/info`
- Version et checksum dans les métadonnées
- Comparaison avec les valeurs attendues (si configurées)

## Tooling Makefile

### Commandes disponibles

#### `make agent-checksum`
Calcule et affiche le SHA256 de `agent.py` :
```bash
$ make agent-checksum
📦 Calculating SHA256 checksum for inventiv-worker/agent.py...
4f9441dc6c913ecb9fda38fd1f9f7413c97df05af3d5fe8c11dd05b312be9ada
```

#### `make agent-version-get`
Affiche la version actuelle :
```bash
$ make agent-version-get
1.0.0
```

#### `make agent-version-bump [VERSION=1.0.1] [BUILD_DATE=2026-01-03]`
Met à jour manuellement la version et la date :
```bash
$ make agent-version-bump VERSION=1.0.1
📝 Updating agent version: 1.0.0 -> 1.0.1
📅 Build date: 2026-01-03 -> 2026-01-03
✅ Updated inventiv-worker/agent.py
```

#### `make agent-version-auto-bump`
Incrémente automatiquement la version patch :
```bash
$ make agent-version-auto-bump
🔄 Auto-bumping version: 1.0.0 -> 1.0.1
```

#### `make agent-version-check`
Vérifie que la version a été mise à jour si `agent.py` a changé :
```bash
$ make agent-version-check
✅ Version updated: 1.0.0 -> 1.0.1
   Build date: 2026-01-03
```

## CI/CD Integration

### GitHub Actions Workflow

#### Workflow CI (`ci.yml`)
- Vérifie automatiquement que la version est à jour si `agent.py` a changé
- Échoue si `agent.py` modifié mais version non mise à jour

#### Workflow Agent Version Bump (`agent-version-bump.yml`)
- Workflow manuel (`workflow_dispatch`)
- Options :
  - Version spécifique ou auto-increment
  - Date de build personnalisée ou date du jour
- Crée automatiquement une Pull Request avec :
  - Nouvelle version
  - SHA256 checksum
  - Instructions pour mettre à jour `WORKER_AGENT_SHA256`

### Workflow recommandé

1. **Modifier `agent.py`** localement
2. **Auto-bump version** : `make agent-version-auto-bump`
3. **Vérifier checksum** : `make agent-checksum`
4. **Commit et push** : `git add inventiv-worker/agent.py && git commit && git push`
5. **CI vérifie** automatiquement que la version est à jour
6. **Mettre à jour `WORKER_AGENT_SHA256`** dans l'environnement de production

## Configuration

### Variables d'environnement

#### `WORKER_AGENT_SOURCE_URL`
URL de téléchargement de `agent.py` :
```bash
WORKER_AGENT_SOURCE_URL=https://raw.githubusercontent.com/Inventiv-IT-for-AI/inventiv-agents/main/inventiv-worker/agent.py
```

#### `WORKER_AGENT_SHA256` (optionnel mais recommandé)
Checksum SHA256 attendu pour vérification d'intégrité :
```bash
WORKER_AGENT_SHA256=4f9441dc6c913ecb9fda38fd1f9f7413c97df05af3d5fe8c11dd05b312be9ada
```

### Provider Settings

Peut être configuré par provider dans `provider_settings` :

```sql
INSERT INTO provider_settings (provider_id, key, value_text)
VALUES ('<provider-id>', 'WORKER_AGENT_SHA256', '4f9441dc...');
```

## Monitoring & Observabilité

### Métadonnées stockées

Les informations agent sont stockées dans :
- **`worker_metadata`** (JSONB) : `agent_info` dans chaque heartbeat
- **`action_logs.metadata`** : Informations agent dans les logs de health check

### Détection de problèmes

Le système détecte automatiquement :
- **Version incorrecte** : Comparaison dans les métadonnées
- **Checksum invalide** : Vérification automatique dans le script bootstrap
- **Agent non accessible** : Erreur loggée si `/info` échoue
- **Agent non démarré** : Absence de heartbeat avec `agent_info`

### Requêtes SQL utiles

```sql
-- Instances avec version agent incorrecte
SELECT id, worker_metadata->'agent_info'->>'version' as agent_version
FROM instances
WHERE worker_metadata->'agent_info'->>'version' != '1.0.0';

-- Instances avec erreur agent_info
SELECT id, worker_metadata->'agent_info_error'
FROM instances
WHERE worker_metadata->'agent_info_error' IS NOT NULL;

-- Dernière version agent utilisée par instance
SELECT 
  id,
  worker_metadata->'agent_info'->>'version' as version,
  worker_metadata->'agent_info'->>'checksum' as checksum,
  worker_last_heartbeat
FROM instances
WHERE worker_metadata->'agent_info' IS NOT NULL
ORDER BY worker_last_heartbeat DESC;
```

## Sécurité

### Garanties

- **Intégrité** : Checksum SHA256 vérifié avant exécution
- **Traçabilité** : Version et checksum loggés dans chaque heartbeat
- **Détection** : Problèmes détectés automatiquement et loggés
- **Rétrocompatibilité** : Fonctionne sans `WORKER_AGENT_SHA256` (avec avertissement)

### Bonnes pratiques

1. **Toujours définir `WORKER_AGENT_SHA256`** en production
2. **Mettre à jour la version** à chaque modification de `agent.py`
3. **Vérifier le checksum** avant de déployer
4. **Surveiller les métadonnées** pour détecter les problèmes

## Code de référence

- **Agent** : `inventiv-worker/agent.py`
- **Health check** : `inventiv-orchestrator/src/health_check_flow.rs`
- **Heartbeat handler** : `inventiv-orchestrator/src/main.rs`
- **Makefile** : Commandes `agent-*`
- **CI/CD** : `.github/workflows/agent-version-bump.yml`

