# Engineering Guidelines (Clean Code & Maintainability)

Objectif: garder le code **lisible**, **maintenable** et **facile à tester** à mesure que le projet grandit.

## Principe clé: “un fichier / un module / une mission”

- Éviter de transformer les fichiers d’entrée / pivots en “God files”
  - Rust: `main.rs`, gros modules “router”
  - Frontend: `page.tsx`, gros composants “écran”
- Un fichier qui:
  - fait du routing + du parsing + des appels DB + de la logique métier + du rendering
  - devient difficile à lire / tester
  → doit être refactoré en unités plus petites.

## Entry points “thin”

### Rust (`main.rs`)

`main.rs` doit rester un **orchestrateur**:
- configuration
- wiring (routes, état)
- démarrage (server, background loops)

À extraire hors de `main.rs`:
- handlers HTTP
- logique métier (services)
- accès DB / queries (repositories / modules dédiés)
- transformations DTO / mapping
- jobs & loops background

### Frontend (`page.tsx`)

`page.tsx` doit rester une **composition**:
- layout / composition de sections
- appels via hooks / services

À extraire hors de `page.tsx`:
- composants UI réutilisables (`ia-widgets` si cross-feature)
- composants feature (ex: `components/instances/*`)
- logique fetch/transform (hooks, `lib/*`)
- parsing/validation (helpers)

## Réutilisabilité: où mettre le code

- **`inventiv-frontend/src/components/ui/*`**: primitives UI shadcn
- **`inventiv-ui/ia-widgets`**: composants réutilisables (préfixe `IA*`)
- **`inventiv-frontend/src/components/*`**: composants feature
- **Rust**: modules dédiés par domaine / responsabilité (`services`, `handlers`, `jobs`, `db`)

## Guidelines pratiques

- **Fonctions courtes** et nommées par intention (lisibles sans commentaire).
- **Éviter la logique conditionnelle en cascade**: extraire en fonctions.
- **Limiter les effets de bord**: isoler IO (DB/HTTP) du calcul pur.
- **Types/DTO**: garder des structs/classements simples; mapper à l’entrée/sortie.
- **Observabilité**: logs structurés, erreurs explicites, pas de `unwrap()` sur chemins runtime.

## Testing / testabilité

- Une logique non-triviale doit être:
  - testée, ou
  - structurée pour être testable (pure functions, séparation IO/calcul)
- Objectif: éviter les tests “end-to-end only” qui deviennent lents et fragiles.

## Signal de refactor (quand agir)

- fichier > ~400–600 lignes et en croissance
- `main.rs` / `page.tsx` commence à contenir de la logique métier
- duplication (copy/paste de patterns)
- changements simples deviennent risqués (effet domino)


