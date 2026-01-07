# Gestion Automatique des Erreurs 401 - Redirection vers Login

## Problème

Les erreurs 401 (Unauthorized) ou l'absence de session ne déclenchaient pas automatiquement une redirection vers la page de login. Chaque composant devait gérer manuellement ces cas, ce qui créait des incohérences.

## Solution

Un wrapper centralisé autour de `fetch()` qui intercepte automatiquement les réponses 401 et redirige vers `/login`.

### Fichiers Créés

1. **`inventiv-frontend/src/lib/api-client.ts`**
   - `apiFetch()` : Wrapper autour de `fetch()` avec gestion automatique des 401
   - `apiRequest()` : Utilise `apiUrl()` + `apiFetch()` pour simplifier les appels
   - `apiJson()` : Helper pour les requêtes JSON avec parsing automatique

### Utilisation

#### Avant (ancien code)
```typescript
const res = await fetch(apiUrl("/auth/me"));
if (!res.ok) {
  if (res.status === 401) {
    router.replace("/login");
    return;
  }
  // ... gestion erreur
}
```

#### Après (nouveau code)
```typescript
import { apiRequest } from "@/lib/api";

const res = await apiRequest("/auth/me");
if (!res.ok) {
  // 401 est géré automatiquement (redirection vers /login)
  // ... gestion autres erreurs
}
```

### Migration Progressive

Les fichiers suivants utilisent encore `fetch(apiUrl(...))` et doivent être migrés :

- `inventiv-frontend/src/app/(app)/settings/page.tsx`
- `inventiv-frontend/src/app/(app)/traces/page.tsx`
- `inventiv-frontend/src/app/(app)/chat/page.tsx`
- `inventiv-frontend/src/app/(app)/api-keys/page.tsx`
- `inventiv-frontend/src/app/(app)/models/page.tsx`
- `inventiv-frontend/src/app/(app)/observability/page.tsx`
- `inventiv-frontend/src/app/(app)/workbench/page.tsx`
- `inventiv-frontend/src/app/(app)/users/page.tsx`
- `inventiv-frontend/src/app/(app)/instances/page.tsx`
- `inventiv-frontend/src/app/(app)/monitoring/page.tsx`

**Migration effectuée** :
- ✅ `inventiv-frontend/src/components/account/AccountSection.tsx`

### Comment Migrer

1. **Remplacer l'import** :
   ```typescript
   // Avant
   import { apiUrl } from "@/lib/api";
   
   // Après
   import { apiUrl, apiRequest } from "@/lib/api";
   ```

2. **Remplacer les appels fetch** :
   ```typescript
   // Avant
   const res = await fetch(apiUrl("/endpoint"), { ... });
   if (!res.ok) {
     if (res.status === 401) {
       router.replace("/login");
       return;
     }
     // ...
   }
   
   // Après
   const res = await apiRequest("/endpoint", { ... });
   if (!res.ok) {
     // 401 géré automatiquement
     // ...
   }
   ```

3. **Supprimer les vérifications 401 manuelles** :
   - Supprimer tous les `if (res.status === 401) router.replace("/login")`
   - Le wrapper s'en charge automatiquement

### Fonctionnalités

- ✅ **Redirection automatique** : Toute réponse 401 redirige vers `/login`
- ✅ **Protection contre les boucles** : Flag `isRedirecting` évite les redirections multiples
- ✅ **Credentials inclus** : `credentials: "include"` pour les cookies de session
- ✅ **Compatible avec fetch** : Même API que `fetch()`, remplacement transparent

### Notes Importantes

1. **Server-side** : Le wrapper ne fonctionne que côté client (`typeof window !== "undefined"`)
2. **Layout protection** : Le layout `(app)/layout.tsx` vérifie déjà le cookie côté serveur
3. **Logout** : Le logout utilise toujours `router.replace("/login")` explicitement (comportement attendu)

### Tests

Pour tester la redirection automatique :

1. Se connecter à l'application
2. Ouvrir la console du navigateur
3. Supprimer manuellement le cookie `inventiv_session`
4. Faire une action qui déclenche un appel API
5. Vérifier que la redirection vers `/login` se fait automatiquement

### Prochaines Étapes

1. Migrer progressivement tous les fichiers listés ci-dessus
2. Ajouter des tests E2E pour vérifier la redirection automatique
3. Documenter dans le guide de développement

