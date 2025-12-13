# Configuration des URLs d'API - Guide

## ⚠️ Problème actuel

Le frontend utilise `/api/backend/...` qui n'existe PAS (pas de proxy Next.js configuré).
Résultat : **Les requêtes POST /deployments n'arrivent jamais au backend !**

## ✅ Solution professionnelle

### 1. Créer `/inventiv-frontend/.env.local`

```bash
# Backend API URL
NEXT_PUBLIC_API_URL=http://localhost:8003
```

### 2. Créer `/inventiv-frontend/src/lib/api.ts`

```typescript
// API configuration 
export const API_BASE_URL = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:8003';

// Helper function
export const apiUrl = (path: string) => `${API_BASE_URL}${path.startsWith('/') ? path : `/${path}`}`;
```

### 3. Modifier les fichiers frontend

#### `src/app/page.tsx`

```typescript
// Ajouter l'import
import { apiUrl } from "@/lib/api";

// Remplacer:
fetch("/api/backend/deployments", ...)
// Par:
fetch(apiUrl("/deployments"), ...)

// Remplacer:
fetch("/api/backend/providers")
// Par:
fetch(apiUrl("/providers"))

// Etc pour toutes les requêtes
```

#### `src/app/settings/page.tsx`

Même principe - remplacer tous les `/api/backend/...` par `apiUrl("...")`

#### `src/app/monitoring/page.tsx`

Idem

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

## 🔍 Debug actuel

Le problème **immédiat** est que `/api/backend/deployments` ne mène nulle part.

**Quick fix temporaire** (pas recommandé) :
```typescript
 fetch("http://localhost:8003/deployments", ...)
```

**Vraie solution** (recommandé) :
Suivre les étapes ci-dessus pour configurer `apiUrl()` proprement.

## 📝 Next Steps

1. ✅ Créer `.env.local` avec `NEXT_PUBLIC_API_URL`
2. ✅ Créer `src/lib/api.ts`  
3. ⏹️ Remplacer tous les `/api/backend/` par `apiUrl("/")` dans :
   - src/app/page.tsx
   - src/app/settings/page.tsx
   - src/app/monitoring/page.tsx
4. ⏹️ Tester la création d'instance
5. ⏹️ Créer `.env.example` avec template
6. ⏹️ Documenter dans README

## 🚀 Redémarrage nécessaire

Après modification des `.env*`, redémarrer le serveur dev :
```bash
cd inventiv-frontend
npm run dev -- -p 3002
```
