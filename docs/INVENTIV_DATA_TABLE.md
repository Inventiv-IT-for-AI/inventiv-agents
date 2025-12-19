# IADataTable — Guide utilisateur

Ce guide décrit comment utiliser le composant React **`IADataTable`** pour afficher des listes virtualisées avec :
- colonnes **masquables / réordonnables / redimensionnables** (préférences persistées par `listId`)
- **tri au clic** sur les en-têtes (client-side ou server-side)

## Import

Le composant vit dans le package monorepo :
- `inventiv-ui/ia-widgets/src/IADataTable.tsx`

Usage :

```tsx
import { IADataTable, type IADataTableColumn } from "ia-widgets";
```

## Concepts clés

- **`listId`**: identifiant stable (string) qui sert à persister les préférences de colonnes (et optionnellement le tri) dans `localStorage`.
  - Recommandation: prefixer par page/module (`"instances:table"`, `"settings:providers"`, etc.)
- **`rows` vs `loadRange`**
  - **`rows`**: données locales en mémoire (tri client possible)
  - **`loadRange(offset, limit)`**: données “remote” paginées (tri typiquement côté serveur)
- **Tri**
  - Une colonne est “triable” si vous lui donnez `sortable: true` et **au moins** un de :
    - `getSortValue(row)` (le plus simple)
    - `sortFn(a, b)` (comparateur custom)
  - Toute la **cellule header** est cliquable pour trier (sauf pendant un resize ou un drag&drop de colonne).

## Exemple 1 — Données locales (`rows`) + tri client

```tsx
type Row = { id: string; name: string; cost?: number; createdAt: string };

const columns: IADataTableColumn<Row>[] = [
  { id: "name", label: "Name", sortable: true, getSortValue: (r) => r.name, cell: ({ row }) => row.name },
  { id: "cost", label: "Cost", sortable: true, getSortValue: (r) => r.cost ?? null, align: "right", cell: ({ row }) => row.cost ?? "-" },
  { id: "created", label: "Created", sortable: true, getSortValue: (r) => new Date(r.createdAt), cell: ({ row }) => r.createdAt },
];

return (
  <IADataTable<Row>
    listId="example:local"
    title="Local table"
    autoHeight
    height={300}
    rowHeight={48}
    columns={columns}
    rows={rows}
    getRowKey={(r) => r.id}
  />
);
```

Notes :
- Le tri est **stable** (à égalité, l’ordre d’origine est conservé).
- Les valeurs `null/undefined` sont gardées **en bas** (tri asc/desc).

## Exemple 2 — Données remote (`loadRange`) + tri server-side

Avec `loadRange`, `IADataTable` est généralement en mode **server** : il **émet** l’état de tri, et c’est au parent (hook/API) de refetch dans le bon ordre.

```tsx
type SortState = { columnId: string; direction: "asc" | "desc" } | null;

const [sort, setSort] = useState<SortState>(null);

const columns: IADataTableColumn<Row>[] = [
  { id: "name", label: "Name", sortable: true, cell: ({ row }) => row.name },
  { id: "created_at", label: "Created", sortable: true, cell: ({ row }) => row.createdAt },
];

async function loadRange(offset: number, limit: number) {
  // Exemple: faire passer sort dans la query string
  const params = new URLSearchParams({
    offset: String(offset),
    limit: String(limit),
    sort_by: sort?.columnId ?? "",
    sort_dir: sort?.direction ?? "",
  });
  const res = await fetch(`/api/backend/your_endpoint?${params.toString()}`);
  return await res.json();
}

return (
  <IADataTable<Row>
    listId="example:remote"
    title="Remote table"
    height={520}
    rowHeight={48}
    columns={columns}
    loadRange={loadRange}
    sortState={sort}
    onSortChange={setSort}
    // Important: forcer le reset du cache virtualisé quand le tri change
    dataKey={`sort:${sort?.columnId ?? "none"}:${sort?.direction ?? "none"}`}
  />
);
```

Bonnes pratiques :
- **Toujours** inclure le tri dans `dataKey` si votre `loadRange` dépend du tri (sinon la virtualisation peut garder des pages en cache dans le mauvais ordre).
- En server-side, `sortable: true` sur une colonne signifie “je veux pouvoir déclencher un tri sur cette colonne”, pas “le tri est automatique”.

## Options de tri

- `defaultSortState`: tri initial (si composant non contrôlé)
- `sortState` + `onSortChange`: mode **contrôlé** (recommandé en server-side)
- `sortCycle`: cycle au clic, par défaut `["asc", "desc", "none"]`
- `persistSort`: persiste l’état de tri dans `localStorage` (scopé par `listId`). **Par défaut: activé**.

## Checklist intégration

- Choisir un `listId` **stable**.
- Fournir `getRowKey` si possible (évite des effets visuels lors des updates).
- Pour le tri client:
  - fournir `getSortValue` (string/number/boolean/Date) ou `sortFn`
- Pour le tri server:
  - gérer `sortState` au niveau page/hook
  - inclure le tri dans `dataKey` pour reset le cache


