# Configuration des URLs d'API - Guide

## ✅ État actuel (repo)

Le frontend utilise un **proxy same-origin** via **`/api/backend/*`** (route handlers Next.js) + le helper **`apiUrl()`** (dans `inventiv-frontend/src/lib/api.ts`).

- **Côté navigateur**: l’UI appelle toujours `GET/POST /api/backend/...` (même origin), ce qui facilite **les cookies de session**.
- **Côté serveur** (SSR / route handlers): la cible upstream est déterminée par:
  - `API_INTERNAL_URL` (prioritaire, utile en Docker/edge: ex `http://api:8003`)
  - sinon `NEXT_PUBLIC_API_URL` (ex `http://localhost:8003` en dev)

## Configuration

### 1. Créer `/inventiv-frontend/.env.local`

```bash
# Backend API URL
NEXT_PUBLIC_API_URL=http://localhost:8003
```

> En local, `make ui` crée automatiquement ce fichier si absent.

### 2. Helper `apiUrl()`

Déjà implémenté dans `inventiv-frontend/src/lib/api.ts`.

### 3. Proxy `/api/backend/*`

Les appels UI passent par:

- `inventiv-frontend/src/app/api/backend/route.ts`
- `inventiv-frontend/src/app/api/backend/[...path]/route.ts`

Ces route handlers proxient la requête vers `API_INTERNAL_URL`/`NEXT_PUBLIC_API_URL` et propagent les cookies.

### 3. Endroits typiques à vérifier

- Dashboard: `inventiv-frontend/src/app/(app)/(dashboard)/page.tsx`
- Instances: `inventiv-frontend/src/app/(app)/instances/page.tsx` + `inventiv-frontend/src/components/instances/*`
- Monitoring: `inventiv-frontend/src/app/(app)/monitoring/page.tsx`
- Traces: `inventiv-frontend/src/app/(app)/traces/page.tsx`
- Settings: `inventiv-frontend/src/app/(app)/settings/page.tsx`
- Login: `inventiv-frontend/src/app/(auth)/login/page.tsx`

### 4. Configuration par environnement

#### Développement local
`.env.local` (gitignored)
```bash
NEXT_PUBLIC_API_URL=http://localhost:8003
```

#### Staging
Exemple `.env.staging` (si tu buildes le frontend hors Docker avec un backend distant):
```bash
NEXT_PUBLIC_API_URL=https://api-staging.yourdomain.com
```

#### Production
Exemple `.env.production` (frontend buildé/déployé séparément):
```bash
NEXT_PUBLIC_API_URL=https://api.yourdomain.com
```

## 🎯 Bénéfices

✅ **Pas de hard-coding** : URLs configurables
✅ **Multi-environnement** : Dev, Staging, Prod
✅ **Facile à déployer** : Juste changer la variable d'env
✅ **Standards Next.js** : Utilise `NEXT_PUBLIC_*` correctement

## 🚀 Redémarrage nécessaire

Après modification des `.env*`, redémarrer le serveur dev :
```bash
cd inventiv-frontend
npm run dev -- --port 3000
```
