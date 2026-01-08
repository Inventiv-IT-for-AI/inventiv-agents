Tu démarres une nouvelle session sur inventiv-agents.

# Objectif: comprendre l’infra LLM (control-plane/data-plane) et les conventions. 

#Lis d’abord README.md et TODO.md, puis :
- docs/architecture.md, 
- docs/domain_design.md, 
- docs/specification_generale.md, 
- docs/API_URL_CONFIGURATION.md, 
- docs/worker_and_router_phase_0_2.md. 

# Explore le code: 
- inventiv-api/src/main.rs, 
- inventiv-orchestrator/src/main.rs, 
- inventiv-worker/agent.py, 
- inventiv-common/. 

# Étudie la DB: sqlx-migrations/ (tables instances, providers, worker_auth_tokens), seeds/catalog_seeds.sql. 

# Vérifie l’exécution locale: 
- make et MakeFile
- docker-compose.yml, 
- scripts/dev_worker_local.sh. 

# Déploiement: 
 deploy/ (nginx/caddy, compose), 
 scripts/scw_instance_provision.sh, 
scripts/deploy_remote.sh. 

# Comprends le bus d’événements Redis (CMD:...) et le loop de Jobs (modules *_job.rs, lancés via tokio::spawn)
- job-health-check (health_check_job::run) : instances booting → transition vers ready / startup_failed
- job-terminator  (terminator_job::run) : instances terminating → confirmation suppression provider → terminated
- job-watch-dog (watch_dog_job::run) : instances ready → vérifie qu’elles existent encore chez le provider (orphan detection)
- job-provisioning (provisioning_job::run) : instances provisioning “stuck” (Redis pub/sub non durable) → requeue provisioning

# Analyse les autres tâches background (pas dans *_job.rs)
- Scaling engine loop: scaling_engine_loop(...) (task background)
- Event listener Redis: subscriber sur orchestrator_events (CMD:PROVISION|TERMINATE|SYNC_CATALOG|RECONCILE) qui spawn des handlers services::*

# Repère les endpoints internes /internal/worker/* et le proxy gateway. 

# Tooling projet (Makefile + commandes make)
- Packaging/Release d’images: make images-build|push|pull + tags immutables (IMAGE_TAG=git sha, IMAGE_TAG_VERSION=v$(VERSION)) et tags “env” par promotion de digest (make images-promote-stg|prod, images-publish-stg|prod).

- Dev local (hot reload): make up/down/ps/logs (= wrappers sur docker-compose.yml + env/dev.env.example).

- Stack “prod-like” locale (edge): make edge-create|start|stop|delete|logs|cert via deploy/docker-compose.nginx.yml (nginx + lego + images prod).

## Staging/Prod remote (Scaleway): make stg-* / make prod-* orchestrent:
- Provision VM (scripts/scw_instance_provision.sh), bootstrap (scripts/remote_bootstrap.sh), sync secrets (scripts/remote_sync_secrets.sh), deploy/update/start/stop/logs (scripts/deploy_remote.sh)

- Gestion certs wildcard via lego (export/import volume dans deploy/certs/).

# Organisation “CI” et environnements Dev/Staging/Prod (Scaleway)
- CI GitHub Actions: .github/workflows/ghcr.yml, sur tag v*: build & push images sur GHCR (arm64) + push aussi un tag SHA

- En workflow_dispatch: promotion vX.Y.Z → staging ou prod (même digest).

- Dev local: docker-compose.yml + env/dev.env.example, services sur localhost, migrations SQLx au boot, AUTO_SEED_CATALOG possible.

- Staging/Prod (chez Scaleway): une VM “control-plane” (db/redis/api/orchestrator/finops/frontend) + edge (nginx/lego ou caddy selon compose).

- Config via env/staging.env.example / env/prod.env.example
secrets montés via SECRETS_DIR (ex: /opt/inventiv/secrets-staging).

# Principes ergonomiques + design system Frontend
- Stack UI: Next.js App Router + Tailwind v4 (globals.css) + shadcn/ui (style “new-york”, components.json) + icônes lucide.

- Composants réutilisables: inventiv-frontend/src/components/ui/* (button, dialog, table, tabs, etc.) + “shared” (StatsCard, VirtualizedDataTable, CopyButton).

- Look & feel: tokens CSS variables (thème clair/sombre) + composants shadcn (cohérence spacing/typography/radius).

- Base URL / endpoints: centralisation via inventiv-frontend/src/lib/api.ts

- Browser-side : appels same-origin vers "/api/backend" (rewrites Next)

- Server-side : API_INTERNAL_URL (docker network) sinon NEXT_PUBLIC_API_URL
construire les URLs via apiUrl(path).

- Types & contrats: src/lib/types.ts + hooks dédiés (src/hooks/useInstances.ts, useFinops.ts, useCatalog.ts) pour isoler fetch/DTO/transformations.

# Pratiques importantes : logs structurés, idempotence migrations/seeds, séparation CQRS, adapters providers (mock/scaleway).

# Clean code / maintenabilité (important)
- Ne pas “grossir” indéfiniment les fichiers d’entrée / fichiers pivots (`main.rs`, `page.tsx`, etc.).
  Objectif: **code lisible, maintenable, testable**.
- Appliquer le principe **SRP** (Single Responsibility Principle) : *un fichier / un module / une mission*.
- Préférer **extraire** la logique dans:
  - Rust: `mod`, services, handlers, modules dédiés + fonctions courtes.
  - Frontend: composants `ui/*` ou `ia-widgets` (si réutilisable), composants feature, hooks, lib/utils.
- Garder `main.rs` / `page.tsx` comme **orchestrateurs** (wiring, routing, composition), pas comme “fourre-tout”.
- Ajouter des tests quand la logique est non-triviale (ou rendre la logique testable via séparation).

# Termine par une carte mentale des flux: UI→API→Redis→Orchestrator→Worker, et des points d’extension.

Si tu constate des divergences ou incoherences, fais des remarques, pose des questions, propose des actions de realignement et de mise a jour.
