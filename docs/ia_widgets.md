# `ia-widgets` (Inventiv UI widgets)

Internal monorepo package that groups **reusable UI components** (cross-feature, potentially cross-projects).

## Location

- Source: `inventiv-ui/ia-widgets`
- Main usage: `inventiv-frontend`

## Naming Rules

- All components exposed by the package follow the **`IA*`** prefix
  - Example: `AIToggle`, `IADataTable`
- Goal: immediately identify "portable UI" components and avoid "app-specific" names (`Inventiv*`, `Active*`, etc.)

## Import / Usage

In `inventiv-frontend`, import from the package:

- `import { IADataTable, AIToggle } from "ia-widgets";`

## Current Content

- **`AIToggle`**: compact toggle (sky accent) for "active/inactive" states.
- **`IADataTable`**: virtualized table (remote/local) with:
  - hideable/reorderable/resizable columns
  - localStorage persistence by `listId`
  - client/server sorting
- **`VirtualizedRemoteList`**, `useAvailableHeight`: internal utilities but exposed as they're used by other components.

## Relationship with `ia-designsys`

- **`ia-designsys`** contains UI primitives (Button/Dialog/Input/Select/Tabs…).
- **`ia-widgets`** contains higher-level reusable widgets that rely on `ia-designsys` (and/or Radix/shadcn primitives).

## When to Add a Component to `ia-widgets`?

✅ Add to `ia-widgets` if:
- used (or planned) in **multiple screens/features**
- stable pattern (not an experiment)
- useful outside the project (or outside the module)

❌ Keep in `inventiv-frontend` if:
- very specific to a feature (e.g., very custom monitoring UI)
- strongly depends on the project's business API

## Addition Process (Convention)

1. Validate the need + style (with the team) **before** implementing a new widget.
2. Implement in `inventiv-ui/ia-widgets/src/*`
3. Export in `inventiv-ui/ia-widgets/src/index.ts`
4. Ensure the component:
   - respects tokens (background/foreground/muted/border)
   - handles focus/disabled/loading
   - doesn't "hardcode" app-specific styles

## Technical Notes (Types)

Today, `inventiv-frontend` transpiles the TS code of the package (monorepo workspaces).

## Dev Loop (Auto-refresh)

We make frequent iterations on `ia-widgets`. In Docker dev (`make ui`), we install at the **root** (npm workspaces) and Next runs in **webpack mode** to have reliable **watch/HMR** on package changes.
