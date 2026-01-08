# Dev Local Setup Checklist

## ✅ Vérifications Automatiques

Exécutez le script de vérification :
```bash
bash scripts/check_dev_setup.sh
```

## 📋 Checklist Manuelle

### 1. Fichiers de Configuration ✅
- [x] `env/dev.env` existe et est configuré
- [x] `docker-compose.yml` présent
- [x] `SECRETS_DIR` pointe vers `./deploy/secrets-dev`

### 2. Secrets Requis

#### Secrets Critiques (nécessaires au démarrage)
- [x] `deploy/secrets-dev/default_admin_password` - Mot de passe admin
- [x] `deploy/secrets-dev/worker_hf_token` - Token HuggingFace pour worker
- [x] `deploy/secrets-dev/scaleway_secret_key` - Clé secrète Scaleway API
- [x] `deploy/secrets-dev/provider_settings_key` - Clé de chiffrement provider settings
- [x] `deploy/secrets-dev/llm-studio-key` - Clé SSH privée
- [x] `deploy/secrets-dev/llm-studio-key.pub` - Clé SSH publique

#### Secrets Optionnels (pour fonctionnalités avancées)
- [ ] `deploy/secrets-dev/scaleway_access_key` - Clé d'accès Scaleway (pour opérations CLI comme volume resize)
  - ⚠️ **Note**: Ce fichier est vide actuellement. Nécessaire uniquement pour certaines opérations Scaleway avancées.

### 3. Variables d'Environnement dans `env/dev.env`

#### ✅ Configurées Correctement
- `SECRETS_DIR=./deploy/secrets-dev`
- `POSTGRES_PASSWORD=password`
- `POSTGRES_DB=llminfra`
- `DEFAULT_ADMIN_USERNAME=admin`
- `DEFAULT_ADMIN_EMAIL=hammed.ramdani@inventiv-it.fr`
- `SCALEWAY_PROJECT_ID=c4c36580-4e0d-4584-83c5-81917932768e`
- `SCALEWAY_ORGANIZATION_ID=bc070744-07ff-40c4-a3c9-5e715ee0d3b7`

#### ⚠️ À Vérifier/Configurer
- `WORKER_CONTROL_PLANE_URL` - URL du tunnel Cloudflare (si vous testez avec des instances Scaleway réelles)
  - Actuellement commenté dans `dev.env`
  - Nécessaire uniquement si `WORKER_AUTO_INSTALL=1` et que vous provisionnez de vraies instances Scaleway

### 4. Configuration SMTP

Les paramètres SMTP sont configurés pour Scaleway Transactional Email :
- `SMTP_SERVER=smtp.tem.scaleway.com`
- `SMTP_PORT=465`
- `SMTP_USERNAME=c4c36580-4e0d-4584-83c5-81917932768e`
- `SMTP_PASSWORD_FILE=/run/secrets/scaleway_secret_key`
- `SMTP_FROM_EMAIL=noreply-dev@inventiv-agents.fr`

⚠️ **Important**: L'adresse email `noreply-dev@inventiv-agents.fr` doit être vérifiée dans Scaleway TEM pour que l'envoi d'emails fonctionne.

### 5. Docker

Vérifiez que Docker est installé et fonctionne :
```bash
docker info
docker compose version
```

## 🚀 Démarrage

Une fois toutes les vérifications passées :

```bash
# Démarrer la stack (DB, Redis, API, Orchestrator, FinOps)
make up

# Dans un autre terminal, démarrer l'UI
make ui
```

L'UI sera accessible sur `http://localhost:3000` (ou `http://localhost:${3000+PORT_OFFSET}` si `PORT_OFFSET` est défini).

## 🔧 Actions Correctives

### Si `SCALEWAY_ACCESS_KEY` est vide

Ce fichier est nécessaire pour certaines opérations Scaleway avancées (comme le redimensionnement de volumes). Pour le remplir :

1. Récupérez votre clé d'accès Scaleway depuis le [console Scaleway](https://console.scaleway.com/iam/api-keys)
2. Ajoutez-la dans `deploy/secrets-dev/scaleway_access_key` :
   ```bash
   echo "votre-clé-d'accès-scaleway" > deploy/secrets-dev/scaleway_access_key
   ```

### Si `WORKER_CONTROL_PLANE_URL` est nécessaire

Si vous testez avec de vraies instances Scaleway et que `WORKER_AUTO_INSTALL=1` :

1. Créez un tunnel Cloudflare (ou utilisez un autre tunnel) :
   ```bash
   cloudflared tunnel --url http://localhost:8003
   ```
2. Mettez à jour `WORKER_CONTROL_PLANE_URL` dans `env/dev.env` avec l'URL du tunnel

### Si les emails SMTP ne fonctionnent pas

1. Vérifiez que l'adresse email `SMTP_FROM_EMAIL` est vérifiée dans Scaleway TEM
2. Vérifiez que `SMTP_PASSWORD_FILE` pointe vers le bon secret (`scaleway_secret_key`)
3. Vérifiez les logs de l'API pour les erreurs SMTP

## 📝 Notes

- Les secrets sont montés dans les conteneurs via `/run/secrets/`
- Le mot de passe admin est lu depuis `/run/secrets/default_admin_password`
- Le token HuggingFace est lu depuis `/run/secrets/worker_hf_token`
- La clé secrète Scaleway est utilisée pour l'API Scaleway ET pour SMTP
