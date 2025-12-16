# Configuration des URLs d'API - Guide

## ✅ État actuel (repo)

Le frontend utilise maintenant **`NEXT_PUBLIC_API_URL`** + le helper **`apiUrl()`** (dans `inventiv-frontend/src/lib/api.ts`).
Cela évite les URLs hardcodées et garantit que l’UI parle toujours au bon backend.

## Configuration

### 1. Créer `/inventiv-frontend/.env.local`

```bash
# Backend API URL
NEXT_PUBLIC_API_URL=http://localhost:8003
```

### 2. Helper `apiUrl()`

Déjà implémenté dans `inventiv-frontend/src/lib/api.ts`.

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
`.env.staging`
```bash
NEXT_PUBLIC_API_URL=https://api-staging.yourdomain.com
```

#### Production
`.env.production`
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
