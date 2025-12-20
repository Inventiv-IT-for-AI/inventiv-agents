## CI / CD (local + GitHub Actions)

### Objectif

- **Local (automatique)**: une commande unique pour reproduire les checks CI.
- **Staging (auto)**: à chaque push sur `main`, si la CI passe, build/push des images + promotion `:staging` + `make stg-update`.
- **Production (manuel)**: promotion `:prod` + `make prod-update`, déclenché manuellement (et idéalement protégé par un “environment approval”).

---

### Local

Depuis la racine du repo:

```bash
make ci-fast
```

Ce que ça fait:
- `cargo fmt --check`
- `cargo clippy` (warnings = erreur)
- `cargo test`
- `npm ci` + `eslint` + `next build`

---

### GitHub Actions: workflows ajoutés

- `CI` → `.github/workflows/ci.yml`
  - Déclenchement: PR + push sur `main`
  - Jobs: Rust (fmt/clippy/test) + Frontend (lint/build)

- `Deploy (staging)` → `.github/workflows/deploy-staging.yml`
  - Déclenchement: push sur `main` (et manuel via `workflow_dispatch`)
  - Étapes:
    - exécute la CI (workflow reusable)
    - build + push images taggées `:<sha12>`
    - promotion vers `:staging` (par digest)
    - déploiement remote via `make stg-update`

- `Deploy (production)` → `.github/workflows/deploy-prod.yml`
  - Déclenchement: **manuel** (`workflow_dispatch`)
  - Input: `image_tag` (ex: `a1b2c3d4e5f6` ou `v0.3.0`)
  - Étapes:
    - promotion `image_tag` → `:prod` (par digest)
    - déploiement remote via `make prod-update`

- `GHCR (build + tag)` → `.github/workflows/ghcr.yml`
  - Déclenchement: push tag `v*`
  - Étapes:
    - build + push `:<sha12>`
    - tag version `:<vX.Y.Z>` (même digest)

---

### Secrets / Environments GitHub requis

Créer 2 environments GitHub:
- `staging`
- `production` (configurer “required reviewers” pour validation manuelle)

Secrets minimaux pour `staging`:
- `STG_REMOTE_HOST`
- `STG_SECRETS_DIR`
- `STG_SSH_PRIVATE_KEY` (clé privée SSH, multi-ligne)
- `STG_POSTGRES_PASSWORD`
- `STG_WORKER_AUTH_TOKEN`
- `STG_ROOT_DOMAIN`
- `STG_FRONTEND_DOMAIN`
- `STG_API_DOMAIN`
- `STG_ACME_EMAIL`

Optionnels (si tu veux surcharger):
- `STG_REMOTE_PORT` (défaut 22)
- `STG_REMOTE_USER` (défaut `ubuntu`)
- `IMAGE_REPO` (défaut `ghcr.io/<owner>/inventiv-agents`)
- `GHCR_USERNAME` (défaut `<owner>`)
- `STG_PROVIDER`, `STG_SCALEWAY_PROJECT_ID`, `STG_RUST_LOG`, `STG_POSTGRES_USER`, `STG_POSTGRES_DB`

Secrets minimaux pour `production` (mêmes champs, préfixe `PROD_`):
- `PROD_REMOTE_HOST`, `PROD_SECRETS_DIR`, `PROD_SSH_PRIVATE_KEY`, `PROD_POSTGRES_PASSWORD`, `PROD_WORKER_AUTH_TOKEN`,
  `PROD_ROOT_DOMAIN`, `PROD_FRONTEND_DOMAIN`, `PROD_API_DOMAIN`, `PROD_ACME_EMAIL`


