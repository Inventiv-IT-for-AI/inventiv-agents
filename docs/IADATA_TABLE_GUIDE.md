# IADataTable — User Guide

This guide describes how to use the React **`IADataTable`** component to display virtualized lists with:
- **hideable / reorderable / resizable** columns (preferences persisted by `listId`)
- **click-to-sort** on headers (client-side or server-side)

## Import

The component lives in the monorepo package:
- `inventiv-ui/ia-widgets/src/IADataTable.tsx`

Usage:

```tsx
import { IADataTable, type IADataTableColumn } from "ia-widgets";
```

## Key Concepts

- **`listId`**: stable identifier (string) used to persist column preferences (and optionally sorting) in `localStorage`.
  - Recommendation: prefix by page/module (`"instances:table"`, `"settings:providers"`, etc.)
- **`rows` vs `loadRange`**
  - **`rows`**: local data in memory (client sorting possible)
  - **`loadRange(offset, limit)`**: paginated "remote" data (sorting typically server-side)
- **Sorting**
  - A column is "sortable" if you give it `sortable: true` and **at least** one of:
    - `getSortValue(row)` (simplest)
    - `sortFn(a, b)` (custom comparator)
  - The entire **header cell** is clickable to sort (except during a resize or column drag&drop).

## Example 1 — Local Data (`rows`) + Client Sorting

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

Notes:
- Sorting is **stable** (on equality, original order is preserved).
- `null/undefined` values are kept **at the bottom** (asc/desc sort).

## Example 2 — Remote Data (`loadRange`) + Server-Side Sorting

With `loadRange`, `IADataTable` is generally in **server** mode: it **emits** the sort state, and it's up to the parent (hook/API) to refetch in the correct order.

```tsx
type SortState = { columnId: string; direction: "asc" | "desc" } | null;

const [sort, setSort] = useState<SortState>(null);

const columns: IADataTableColumn<Row>[] = [
  { id: "name", label: "Name", sortable: true, cell: ({ row }) => row.name },
  { id: "created_at", label: "Created", sortable: true, cell: ({ row }) => row.createdAt },
];

async function loadRange(offset: number, limit: number) {
  // Example: pass sort in query string
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
    // Important: force reset of virtualized cache when sort changes
    dataKey={`sort:${sort?.columnId ?? "none"}:${sort?.direction ?? "none"}`}
  />
);
```

Best practices:
- **Always** include sorting in `dataKey` if your `loadRange` depends on sorting (otherwise virtualization may keep pages cached in the wrong order).
- In server-side mode, `sortable: true` on a column means "I want to be able to trigger sorting on this column", not "sorting is automatic".

## Sorting Options

- `defaultSortState`: initial sort (if component is uncontrolled)
- `sortState` + `onSortChange`: **controlled** mode (recommended for server-side)
- `sortCycle`: click cycle, default `["asc", "desc", "none"]`
- `persistSort`: persists sort state in `localStorage` (scoped by `listId`). **Default: enabled**.

## Integration Checklist

- Choose a **stable** `listId`.
- Provide `getRowKey` if possible (avoids visual effects during updates).
- For client sorting:
  - provide `getSortValue` (string/number/boolean/Date) or `sortFn`
- For server sorting:
  - manage `sortState` at page/hook level
  - include sorting in `dataKey` to reset cache

