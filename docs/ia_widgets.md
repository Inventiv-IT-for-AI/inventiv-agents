# `ia-widgets` (Inventiv UI widgets)

Package interne monorepo qui regroupe les **composants UI réutilisables** (cross-feature, potentiellement cross-projets).

## Emplacement

- Source: `inventiv-ui/ia-widgets`
- Utilisation principale: `inventiv-frontend`

## Règles de naming

- Tous les composants exposés par le package suivent le préfixe **`IA*`**
  - Exemple: `AIToggle`, `IADataTable`
- Objectif: identifier immédiatement les composants “portable UI” et éviter les noms “app-specific” (`Inventiv*`, `Active*`, etc.)

## Import / usage

Dans `inventiv-frontend`, on importe depuis le package:

- `import { IADataTable, AIToggle } from "ia-widgets";`

## Contenu actuel

- **`AIToggle`**: toggle compact (accent sky) pour états “active/inactive”.
- **`IADataTable`**: table virtualisée (remote/local) avec:
  - colonnes masquables/réordonnables/redimensionnables
  - persistance localStorage par `listId`
  - tri client/serveur
- **`VirtualizedRemoteList`**, `useAvailableHeight`: utilitaires internes mais exposés car utilisés par d’autres composants.

## Relation avec `ia-designsys`

- **`ia-designsys`** contient les primitives UI (Button/Dialog/Input/Select/Tabs…).
- **`ia-widgets`** contient des widgets plus haut niveau et réutilisables, qui s’appuient sur `ia-designsys` (et/ou sur des primitives Radix/shadcn).

## Quand ajouter un composant à `ia-widgets` ?

✅ Ajoute dans `ia-widgets` si:
- utilisé (ou prévu) dans **plusieurs écrans/features**
- pattern stable (pas une expérimentation)
- utile hors du projet (ou hors module)

❌ Laisse dans `inventiv-frontend` si:
- très spécifique à une feature (ex: UI monitoring très custom)
- dépend fortement de l’API métier du projet

## Process d’ajout (convention)

1. Valider le besoin + style (avec l’équipe) **avant** d’implémenter un nouveau widget.
2. Implémenter dans `inventiv-ui/ia-widgets/src/*`
3. Exporter dans `inventiv-ui/ia-widgets/src/index.ts`
4. S’assurer que le composant:
   - respecte les tokens (background/foreground/muted/border)
   - gère focus/disabled/loading
   - ne “hardcode” pas des styles app-specific

## Notes techniques (types)

Aujourd’hui, `inventiv-frontend` transpile le code TS du package (workspaces monorepo).

## Dev loop (auto-refresh)

Nous faisons des itérations fréquentes sur `ia-widgets`. En dev Docker (`make ui`), on installe au **root** (npm workspaces) et Next tourne en **mode webpack** afin d’avoir un **watch/HMR fiable** sur les changements du package.


