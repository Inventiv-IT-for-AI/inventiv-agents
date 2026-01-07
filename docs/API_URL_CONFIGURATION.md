# API URL Configuration - Guide

## ✅ Current State (repo)

The frontend supports 2 modes:

1) **Recommended (UI in Docker, UI-only exposed)**  
The browser only talks to the UI (port 3000 + offset). Backend calls go **same-origin** via `/api/backend/*` (Next.js routes) which proxy server-side to `API_INTERNAL_URL=http://api:8003` (Docker network).

2) **UI on host (debug)**  
The browser calls the API directly via `NEXT_PUBLIC_API_URL` (then the API must be exposed on the host, e.g., `make api-expose`).

## Configuration

### Recommended Mode: UI in Docker

- Startup:

```bash
make up
make ui
```

- By default, the API is **not** exposed on the host.

### Host Mode: UI on host (debug)

- Expose API on loopback:

```bash
make api-expose
```

- Then create `inventiv-frontend/.env.local`:

```bash
NEXT_PUBLIC_API_URL=http://127.0.0.1:8003
```

> Note: if you use `PORT_OFFSET`, the exposed API becomes `8003 + PORT_OFFSET` (e.g., `18003`).

### Helper `apiUrl()`

The `apiUrl()` helper is centralized in `inventiv-frontend/src/lib/api.ts` to avoid hardcoded URLs.

## 🎯 Benefits

✅ **No hard-coding**: Configurable URLs
✅ **Multi-environment**: Dev, Staging, Prod
✅ **Easy to deploy**: Just change the env variable
✅ **Next.js standards**: Uses `NEXT_PUBLIC_*` correctly

## 🚀 Restart Required

After modifying `.env*`, restart the dev server (recommended):
```bash
make ui-down
make ui
```

If you use UI on host:

```bash
make ui-local-down
make ui-local
```
