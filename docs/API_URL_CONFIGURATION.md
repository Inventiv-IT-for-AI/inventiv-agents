# Configuration des URLs d'API - Guide

## ✅ État actuel (repo)

Le frontend supporte 2 modes :

1) **Recommandé (UI dans Docker, UI-only exposée)**  
Le navigateur parle uniquement à l’UI (port 3000 + offset). Les appels backend passent en **same-origin** via `/api/backend/*` (routes Next.js) qui proxy côté serveur vers `API_INTERNAL_URL=http://api:8003` (réseau Docker).

2) **UI sur le host (debug)**  
Le navigateur appelle directement l’API via `NEXT_PUBLIC_API_URL` (il faut alors exposer l’API sur le host, ex: `make api-expose`).

## Configuration

### Mode recommandé: UI dans Docker

- Démarrage:

```bash
make up
make ui
```

- Par défaut, l’API n’est **pas** exposée sur le host.

### Mode host: UI sur le host (debug)

- Exposer l’API en loopback:

```bash
make api-expose
```

- Puis créer `inventiv-frontend/.env.local` :

```bash
NEXT_PUBLIC_API_URL=http://127.0.0.1:8003
```

> Note: si tu utilises `PORT_OFFSET`, l’API exposée devient `8003 + PORT_OFFSET` (ex: `18003`).

### Helper `apiUrl()`

Le helper `apiUrl()` est centralisé dans `inventiv-frontend/src/lib/api.ts` pour éviter les URLs hardcodées.

## 🎯 Bénéfices

✅ **Pas de hard-coding** : URLs configurables
✅ **Multi-environnement** : Dev, Staging, Prod
✅ **Facile à déployer** : Juste changer la variable d'env
✅ **Standards Next.js** : Utilise `NEXT_PUBLIC_*` correctement

## 🚀 Redémarrage nécessaire

Après modification des `.env*`, redémarrer le serveur dev (recommandé) :
```bash
make ui-down
make ui
```

Si tu utilises l’UI sur le host :

```bash
make ui-local-down
make ui-local
```
