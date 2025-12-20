# UI Design System (Inventiv Agents)

Ce document décrit **la charte UI** et **les conventions de composants** pour maintenir une interface cohérente, réutilisable et “design-system first”.

## Objectifs

- **Cohérence visuelle**: mêmes patterns, mêmes composants, mêmes tokens.
- **Réutilisabilité**: composants réutilisables regroupés dans le package `ia-widgets`.
- **Évolutivité**: ajouter des composants sans “drift” visuel ni divergence de styles.

> Pour les règles de **maintenabilité/clean code** (éviter les “god files” type `page.tsx`), voir : `docs/engineering_guidelines.md`.

## Stack UI (référence)

- **Next.js (App Router)**: `inventiv-frontend`
- **Tailwind CSS v4**: classes utilitaires + tokens via CSS variables
- **`ia-designsys`**: primitives UI (Button, Dialog, Input, Select, Tabs, etc.) centralisées (shadcn/ui style)
- **`ia-widgets`**: widgets de plus haut niveau (table, modals, copy, etc.)
- **Radix UI**: primitives accessibles (Dialog, Select, Switch, etc.)

## Règle d’or: ne pas inventer de nouveaux widgets sans validation

- **Par défaut**: réutiliser les composants existants (`shadcn/ui`, `ia-widgets`, `shared` existants).
- **Si un besoin UI est nouveau**: on valide d’abord **le besoin**, **le style**, **l’état (states)** et **l’accessibilité**.
- Une fois validé: on implémente dans `ia-widgets` (si réutilisable) ou dans `inventiv-frontend/src/components/ui/*` (si pattern shadcn local au produit).

## Fondations visuelles

- **Couleurs & tokens**: utiliser les classes Tailwind basées sur les variables (ex: `bg-background`, `text-foreground`, `text-muted-foreground`, `border`, `ring-*`).
- **Spacing**: préférer les échelles Tailwind (`gap-2`, `p-4`, `space-y-6`, etc.), éviter les valeurs “custom” sauf nécessité.
- **Typographie**: privilégier `text-sm`, `text-base`, `text-lg`, `font-medium`, `font-semibold`.
- **États**: gérer systématiquement `hover`, `focus-visible`, `disabled`, `aria-invalid`, `data-[state=*]`.

## Composants: où mettre quoi ?

### 1) `inventiv-frontend/src/components/ui/*` (shadcn/ui)

À utiliser pour des primitives UI (Button, Dialog, Input, Select, Tabs, etc.) qui suivent le style shadcn.

### 2) `inventiv-ui/ia-widgets` (réutilisable multi-projets)

À utiliser pour des composants **réutilisables**, orientés “IA platform”, et potentiellement ré-extractables.

- Naming: **préfixe `IA*`** (ex: `IADataTable`, `AIToggle`)
- Import côté app: `import { IADataTable } from "ia-widgets";`

### 3) `inventiv-frontend/src/components/*` (feature/components)

Composants spécifiques à une feature (instances, settings, monitoring…), non destinés à être réutilisés hors du projet.

## Patterns validés (exemples)

- **Toggle**: utiliser `AIToggle` (pattern compact, accent sky) ou `Switch` shadcn selon le besoin.
- **Tables**:
  - `IADataTable` pour tables virtualisées avec colonnes configurables + persistance locale.
  - éviter des implémentations ad-hoc de tables.
- **Modals**: `Dialog` shadcn, header/footer standardisés.
- **Actions**: `Button` + `variant` (`default`, `outline`, `secondary`, `destructive`, `ghost`) au lieu de `<button className="...">`.

## Multi-tenant UX : indicateurs de scope (Personal vs Organisation)

Règle : le **workspace courant** (Personal vs Organisation) change le **scope** de toutes les actions.
Le design doit rendre ce changement **visuellement évident**.

### Sidebar background “org color” (anti-erreur)
- Chaque organisation doit pouvoir définir une **couleur / thème visuel** (MVP: fond de la sidebar).
- Quand l’utilisateur change d’organisation, la sidebar doit changer de couleur immédiatement.
- La couleur doit être suffisamment contrastée pour rester lisible (texte/icônes).
- En mode **Personal**, utiliser le thème neutre par défaut.

> Note: l’objectif est la prévention d’erreur (“je pensais être dans l’org A mais j’étais dans l’org B”).

## Accessibilité (checklist)

- Tous les controls doivent avoir un **label** (`<Label>`, `aria-label`, `aria-describedby`).
- Utiliser `focus-visible` et éviter de “cacher” le focus.
- Ne pas dépendre uniquement de la couleur (ajouter texte/icône).

## Review checklist (PR)

- Aucun nouveau composant “widget” non validé.
- Usage systématique de `Button`, `Dialog`, `Input`, `Select`, `Switch`/`AIToggle`.
- Tokens (background/foreground/muted/border) respectés.
- États UI complets (loading/error/empty/disabled).
- Accessibilité OK (labels, focus, roles).


