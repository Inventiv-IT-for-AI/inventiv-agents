# Configuration de WORKER_CONTROL_PLANE_URL

## Vue d'ensemble

`WORKER_CONTROL_PLANE_URL` est l'URL publique que les workers (instances Scaleway) utilisent pour communiquer avec le control-plane. Cette URL doit être accessible depuis Internet car les instances Scaleway ne peuvent pas accéder directement à votre machine locale.

## Architecture

```
Worker (Scaleway VM)
    ↓ HTTPS
Cloudflare Tunnel (cloudflared)
    ↓ HTTP
localhost:18003 (ou 8003+PORT_OFFSET)
    ↓ Docker Network
API (inventiv-api:8003)
    ↓ Proxy
Orchestrator (inventiv-orchestrator)
```

## Configuration étape par étape

### 1. Exposer l'API localement

L'API doit être accessible sur `localhost` pour que cloudflared puisse créer le tunnel.

**Option A : Utiliser `make api-expose` (recommandé)**

```bash
# Avec PORT_OFFSET=0 (par défaut)
make api-expose

# Avec PORT_OFFSET=10000
PORT_OFFSET=10000 make api-expose
```

Cela expose l'API sur `http://127.0.0.1:8003` (ou `http://127.0.0.1:18003` si PORT_OFFSET=10000).

**Option B : Modifier docker-compose.yml**

Ajouter un port mapping pour l'API :
```yaml
api:
  ports:
    - "${API_HOST_PORT:-8003}:8003"
```

### 2. Créer le tunnel Cloudflare

```bash
# Si PORT_OFFSET=0
cloudflared tunnel --url http://127.0.0.1:8003

# Si PORT_OFFSET=10000
cloudflared tunnel --url http://127.0.0.1:18003
```

**Important** : Gardez ce terminal ouvert ! Le tunnel doit rester actif.

### 3. Récupérer l'URL du tunnel

Cloudflared affiche une URL du type :
```
https://xxxxx-xxxxx-xxxxx.trycloudflare.com
```

**Note** : Cette URL change à chaque redémarrage de cloudflared. Pour une URL stable, utilisez un tunnel nommé Cloudflare.

### 4. Configurer WORKER_CONTROL_PLANE_URL

Dans `env/dev.env`, mettez l'URL complète du tunnel :

```bash
WORKER_CONTROL_PLANE_URL=https://xxxxx-xxxxx-xxxxx.trycloudflare.com
```

**Important** :
- Ne pas ajouter de slash à la fin (`/`)
- Ne pas ajouter de chemin (`/internal/worker/...`)
- Utiliser `https://` (cloudflared utilise toujours HTTPS)

### 5. Vérifier la configuration

Le worker appellera :
- `POST https://xxxxx-xxxxx-xxxxx.trycloudflare.com/internal/worker/register`
- `POST https://xxxxx-xxxxx-xxxxx.trycloudflare.com/internal/worker/heartbeat`

Vous pouvez tester manuellement :
```bash
curl -X POST https://xxxxx-xxxxx-xxxxx.trycloudflare.com/internal/worker/register \
  -H "Content-Type: application/json" \
  -d '{"instance_id":"test","worker_id":"test"}'
```

## Configuration avec PORT_OFFSET

Si vous utilisez `PORT_OFFSET=10000` :

1. **Exposer l'API** :
   ```bash
   PORT_OFFSET=10000 make api-expose
   ```
   L'API sera accessible sur `http://127.0.0.1:18003`

2. **Créer le tunnel** :
   ```bash
   cloudflared tunnel --url http://127.0.0.1:18003
   ```

3. **Configurer WORKER_CONTROL_PLANE_URL** avec l'URL du tunnel

## Tunnel Cloudflare permanent (optionnel)

Pour une URL stable qui ne change pas à chaque redémarrage :

1. Créer un tunnel nommé dans Cloudflare
2. Configurer le tunnel pour pointer vers `localhost:8003` (ou `18003` avec PORT_OFFSET)
3. Utiliser l'URL du tunnel nommé dans `WORKER_CONTROL_PLANE_URL`

Voir la [documentation Cloudflare](https://developers.cloudflare.com/cloudflare-one/connections/connect-apps/) pour plus de détails.

## Dépannage

### Le worker ne peut pas se connecter

1. Vérifiez que cloudflared est toujours actif
2. Vérifiez que l'API est accessible localement :
   ```bash
   curl http://127.0.0.1:8003/
   ```
3. Vérifiez que le tunnel fonctionne :
   ```bash
   curl https://xxxxx-xxxxx-xxxxx.trycloudflare.com/
   ```
4. Vérifiez les logs du worker sur l'instance Scaleway

### L'URL change à chaque redémarrage

Utilisez un tunnel Cloudflare nommé pour une URL stable.

### Erreur "Connection refused"

- Vérifiez que `make api-expose` a été exécuté
- Vérifiez que le port correspond à celui utilisé par cloudflared
- Vérifiez que l'API est démarrée (`make up`)
