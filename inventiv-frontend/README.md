# inventiv-frontend

Console web Inventiv Agents (Next.js App Router).

## Dev local (recommandé)

Depuis la racine du repo:

```bash
make ui
```

Le frontend tourne dans Docker et proxifie le backend via des routes same-origin:

- `/api/backend/*` → `API_INTERNAL_URL=http://api:8003`

## Monorepo (npm workspaces)

Le repo utilise **npm workspaces** pour partager des packages TS/React comme `inventiv-ui/ia-widgets`.
En local hors Docker, préfère installer au root:

```bash
npm install
npm -w inventiv-frontend run dev
```

> Important (Tailwind v4 / CSS-first): le frontend référence explicitement les sources des packages workspaces via `@source` dans `src/app/globals.css` afin que les classes Tailwind utilisées dans `ia-widgets`/`ia-designsys` ne soient pas purgées.

## Design system

- Charte & conventions: `../docs/ui_design_system.md`
- Widgets réutilisables monorepo: `../inventiv-ui/ia-widgets` (import: `ia-widgets`)
