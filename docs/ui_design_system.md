# UI Design System (Inventiv Agents)

This document describes **the UI charter** and **component conventions** to maintain a consistent, reusable, and "design-system first" interface.

## Objectives

- **Visual consistency**: same patterns, same components, same tokens.
- **Reusability**: reusable components grouped in the `ia-widgets` package.
- **Scalability**: add components without "visual drift" or style divergence.

> For **maintainability/clean code** rules (avoid "god files" like `page.tsx`), see: `docs/engineering_guidelines.md`.

## UI Stack (reference)

- **Next.js (App Router)**: `inventiv-frontend`
- **Tailwind CSS v4**: utility classes + tokens via CSS variables
- **`ia-designsys`**: centralized UI primitives (Button, Dialog, Input, Select, Tabs, etc.) (shadcn/ui style)
- **`ia-widgets`**: higher-level widgets (table, modals, copy, etc.)
- **Radix UI**: accessible primitives (Dialog, Select, Switch, etc.)

## Golden rule: don't invent new widgets without validation

- **By default**: reuse existing components (`shadcn/ui`, `ia-widgets`, existing `shared`).
- **If a UI need is new**: validate first **the need**, **the style**, **the state (states)** and **accessibility**.
- Once validated: implement in `ia-widgets` (if reusable) or in `inventiv-frontend/src/components/ui/*` (if shadcn pattern local to the product).

## Visual foundations

- **Colors & tokens**: use Tailwind classes based on variables (e.g., `bg-background`, `text-foreground`, `text-muted-foreground`, `border`, `ring-*`).
- **Spacing**: prefer Tailwind scales (`gap-2`, `p-4`, `space-y-6`, etc.), avoid "custom" values unless necessary.
- **Typography**: prefer `text-sm`, `text-base`, `text-lg`, `font-medium`, `font-semibold`.
- **States**: systematically handle `hover`, `focus-visible`, `disabled`, `aria-invalid`, `data-[state=*]`.

## Components: where to put what?

### 1) `inventiv-frontend/src/components/ui/*` (shadcn/ui)

Use for UI primitives (Button, Dialog, Input, Select, Tabs, etc.) that follow the shadcn style.

### 2) `inventiv-ui/ia-widgets` (reusable multi-projects)

Use for **reusable** components, oriented "IA platform", and potentially re-extractable.

- Naming: **prefix `IA*`** (e.g., `IADataTable`, `AIToggle`)
- Import in app: `import { IADataTable } from "ia-widgets";`

### 3) `inventiv-frontend/src/components/*` (feature/components)

Components specific to a feature (instances, settings, monitoring…), not intended to be reused outside the project.

## Validated patterns (examples)

- **Toggle**: use `AIToggle` (compact pattern, sky accent) or `Switch` shadcn depending on need.
- **Tables**:
  - `IADataTable` for virtualized tables with configurable columns + local persistence.
  - avoid ad-hoc table implementations.
- **Modals**: `Dialog` shadcn, standardized header/footer.
- **Actions**: `Button` + `variant` (`default`, `outline`, `secondary`, `destructive`, `ghost`) instead of `<button className="...">`.

## Multi-tenant UX: scope indicators (Personal vs Organization)

Rule: the **current workspace** (Personal vs Organization) changes the **scope** of all actions.
The design must make this change **visually obvious**.

### Sidebar background "org color" (error prevention)
- Each organization must be able to define a **color / visual theme** (MVP: sidebar background).
- When the user changes organization, the sidebar must change color immediately.
- The color must be sufficiently contrasted to remain readable (text/icons).
- In **Personal** mode, use the default neutral theme.

> Note: the goal is error prevention ("I thought I was in org A but I was in org B").

## Accessibility (checklist)

- All controls must have a **label** (`<Label>`, `aria-label`, `aria-describedby`).
- Use `focus-visible` and avoid "hiding" focus.
- Don't rely solely on color (add text/icon).

## Review checklist (PR)

- No new "widget" component without validation.
- Systematic use of `Button`, `Dialog`, `Input`, `Select`, `Switch`/`AIToggle`.
- Tokens (background/foreground/muted/border) respected.
- Complete UI states (loading/error/empty/disabled).
- Accessibility OK (labels, focus, roles).
