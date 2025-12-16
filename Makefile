VERSION := $(shell cat VERSION)
GIT_SHA := $(shell git rev-parse --short=12 HEAD)

# Port offset (for running multiple worktrees side-by-side on the same host).
# Example:
#   make PORT_OFFSET=0 up ui          -> UI on 3000
#   make PORT_OFFSET=10000 up ui      -> UI on 13000 (no collisions)
PORT_OFFSET ?= 0

# UI host port (computed from PORT_OFFSET by default)
UI_HOST_PORT ?= $(shell off="$(PORT_OFFSET)"; if [ -z "$$off" ]; then off=0; fi; echo $$((3000 + $$off)))

# Container promotion (immutable tags)
IMAGE_TAG ?= $(GIT_SHA)
# Convenience tags (opt-in)
IMAGE_TAG_VERSION ?= v$(VERSION)
IMAGE_TAG_LATEST ?= latest
# Docker requires lowercase repository names
IMAGE_REPO ?= ghcr.io/inventiv-it-for-ai/inventiv-agents

# Compose
# - LOCAL: developer stack (hot reload, docker-compose.yml in repo root)
# - DEPLOY: prod-like stack (nginx/lego + prod Dockerfiles), used for staging/prod and local edge testing
LOCAL_COMPOSE_FILE ?= docker-compose.yml
DEPLOY_COMPOSE_FILE ?= deploy/docker-compose.nginx.yml
COMPOSE ?= docker compose

# Local env files (not committed) should live in env/*.env
# Defaults to *.env.example so commands remain runnable on fresh clones.
DEV_ENV_FILE ?= env/dev.env
STG_ENV_FILE ?= env/staging.env
PRD_ENV_FILE ?= env/prod.env

# Load dev env variables into Make (optional).
# This allows setting PORT_OFFSET (and other non-secret defaults) per worktree
# without passing them on every command line.
-include $(DEV_ENV_FILE)

# Fail fast with helpful guidance when env files are missing.
define require_env_file
	@if [ ! -f "$(1)" ]; then \
	  echo ""; \
	  echo "❌ Missing env file: $(1)"; \
	  echo "Create it from the example:"; \
	  echo "  cp $(2) $(1)"; \
	  echo "Then edit $(1) and retry."; \
	  echo ""; \
	  exit 2; \
	fi
endef

.PHONY: check-dev-env check-stg-env check-prod-env
check-dev-env:
	$(call require_env_file,$(DEV_ENV_FILE),env/dev.env.example)

check-stg-env:
	$(call require_env_file,$(STG_ENV_FILE),env/staging.env.example)

check-prod-env:
	$(call require_env_file,$(PRD_ENV_FILE),env/prod.env.example)

# Compose wrappers (keep commands consistent everywhere)
COMPOSE_LOCAL = $(COMPOSE) -f $(LOCAL_COMPOSE_FILE) --env-file $(DEV_ENV_FILE)
COMPOSE_DEPLOY = $(COMPOSE) -f $(DEPLOY_COMPOSE_FILE) --env-file $(DEV_ENV_FILE)

# Remote deploy
REMOTE_DIR ?= /opt/inventiv-agents
STG_REMOTE_SSH ?=
PRD_REMOTE_SSH ?=

.PHONY: help \
	images-build images-push images-pull images-promote-stg images-promote-prod \
	images-build-version images-push-version images-build-latest images-push-latest \
	images-publish-stg images-publish-prod \
	ghcr-token-local ghcr-login \
	up down ps logs ui dev-create dev-create-edge dev-start dev-start-edge dev-stop dev-delete dev-ps dev-logs dev-restart-orchestrator dev-cert \
	edge-create edge-start edge-stop edge-delete edge-ps edge-logs edge-cert \
	stg-provision stg-destroy stg-bootstrap stg-secrets-sync stg-rebuild stg-create stg-update stg-start stg-stop stg-delete stg-status stg-logs stg-cert stg-renew stg-ghcr-token \
	stg-cert-export stg-cert-import stg-reset-admin-password \
	prod-provision prod-destroy prod-bootstrap prod-secrets-sync prod-rebuild prod-create prod-update prod-start prod-stop prod-delete prod-status prod-logs prod-cert prod-renew prod-ghcr-token \
	prod-cert-export prod-cert-import prod-reset-admin-password \
	reset-admin-password \
	test clean

help:
	@echo ""
	@echo "Inventiv-Agents tooling (container promotion)"
	@echo ""
	@echo "Common vars:"
	@echo "  IMAGE_REPO=$(IMAGE_REPO)"
	@echo "  IMAGE_TAG (default)=$(IMAGE_TAG)   | version tag=$(IMAGE_TAG_VERSION)   | latest tag=$(IMAGE_TAG_LATEST)"
	@echo "  LOCAL_COMPOSE_FILE=$(LOCAL_COMPOSE_FILE)"
	@echo "  DEPLOY_COMPOSE_FILE=$(DEPLOY_COMPOSE_FILE)"
	@echo ""
	@echo "## Images (build/push/pull)"
	@echo "  make images-build [IMAGE_TAG=<sha>]"
	@echo "  make images-push  [IMAGE_TAG=<sha>]"
	@echo "  make images-pull  [IMAGE_TAG=<sha>]"
	@echo "  make images-build-version    # IMAGE_TAG=v$(VERSION)"
	@echo "  make images-push-version     # IMAGE_TAG=v$(VERSION)"
	@echo "  make images-build-latest     # IMAGE_TAG=latest (dev only)"
	@echo "  make images-push-latest      # IMAGE_TAG=latest (dev only)"
	@echo "  make ghcr-token-local        # writes/validates deploy/secrets/ghcr_token (gitignored)"
	@echo ""
	@echo "## Promotion (same digest)"
	@echo "  make images-promote-stg  IMAGE_TAG=<sha|vX.Y.Z>"
	@echo "  make images-promote-prod IMAGE_TAG=<sha|vX.Y.Z>"
	@echo "  make images-publish-stg  # build+push v$(VERSION) then retag to :staging"
	@echo "  make images-publish-prod # build+push v$(VERSION) then retag to :prod"
	@echo ""
	@echo "## DEV local (docker-compose.yml — hot reload)"
	@echo "  make up | down | ps | logs"
	@echo "  make ui            # start Next.js UI on http://localhost:3000"
	@echo "  make dev-create | dev-start | dev-stop | dev-delete"
	@echo "  make dev-restart-orchestrator"
	@echo ""
	@echo "## Local prod-like (deploy/docker-compose.nginx.yml — nginx/lego + prod Dockerfiles)"
	@echo "  make edge-create | edge-start | edge-stop | edge-delete | edge-logs | edge-cert"
	@echo ""
	@echo "## STAGING remote (Scaleway)"
	@echo "  make stg-provision     # create/reuse server in fr-par-2 + attach flex IP + SSH test"
	@echo "  make stg-destroy       # delete server (keeps flexible IP reserved)"
	@echo "  make stg-bootstrap     # install docker/compose + prepare dirs on VM"
	@echo "  make stg-secrets-sync  # upload required secrets to SECRETS_DIR on the VM"
	@echo "  make stg-rebuild       # provision + bootstrap + secrets-sync + deploy"
	@echo "  make stg-create        # rsync deploy/ + upload env + pull + (edge cert) + up -d"
	@echo "  make stg-update        # pull + renew cert + up -d"
	@echo "  make stg-status | stg-logs | stg-stop | stg-delete"
	@echo "  make stg-cert | stg-renew"
	@echo "  make stg-cert-export        # export wildcard cert cache to deploy/certs/"
	@echo "  make stg-cert-import        # import wildcard cert cache from deploy/certs/ to VM"
	@echo "  make stg-reset-admin-password  # reset admin password on staging"
	@echo ""
	@echo "## PROD remote (Scaleway)"
	@echo "  make prod-provision | prod-destroy | prod-bootstrap | prod-secrets-sync | prod-rebuild | prod-create | prod-update"
	@echo "  make prod-status | prod-logs | prod-stop | prod-delete"
	@echo "  make prod-cert | prod-renew"
	@echo "  make prod-cert-export       # export wildcard cert cache to deploy/certs/"
	@echo "  make prod-cert-import       # import wildcard cert cache from deploy/certs/ to VM"
	@echo "  make prod-reset-admin-password  # reset admin password on prod"
	@echo ""
	@echo "Notes:"
	@echo "  - Remote connection settings live in env/staging.env and env/prod.env (REMOTE_HOST/PORT, SSH_IDENTITY_FILE, optional REMOTE_USER)."
	@echo "  - Secrets must exist on the VM under SECRETS_DIR (scaleway_access_key, scaleway_secret_key, llm-studio-key.pub, ghcr_token if GHCR is private)."
	@echo "  - To upload GHCR token to VM: make stg-ghcr-token (or prod-ghcr-token)."
	@echo ""

## -----------------------------
## Images (build/push/pull/promo)
## -----------------------------

images-build:
	@echo "🏗  Building images (tag=$(IMAGE_TAG))"
	IMAGE_TAG=$(IMAGE_TAG) IMAGE_REPO=$(IMAGE_REPO) \
	  $(COMPOSE) -f $(DEPLOY_COMPOSE_FILE) --env-file $(DEV_ENV_FILE) build api orchestrator finops frontend

images-build-version:
	@$(MAKE) images-build IMAGE_TAG=$(IMAGE_TAG_VERSION)

images-push-version:
	@$(MAKE) images-push IMAGE_TAG=$(IMAGE_TAG_VERSION)

images-build-latest:
	@$(MAKE) images-build IMAGE_TAG=$(IMAGE_TAG_LATEST)

images-push-latest:
	@$(MAKE) images-push IMAGE_TAG=$(IMAGE_TAG_LATEST)

images-publish-stg:
	@echo "🚢 Publishing images to :staging"
	@$(MAKE) images-build-version
	@$(MAKE) images-push-version
	@$(MAKE) images-promote-stg IMAGE_TAG=$(IMAGE_TAG_VERSION)

images-publish-prod:
	@echo "🚢 Publishing images to :prod"
	@$(MAKE) images-build-version
	@$(MAKE) images-push-version
	@$(MAKE) images-promote-prod IMAGE_TAG=$(IMAGE_TAG_VERSION)

ghcr-token-local:
	@chmod +x ./scripts/ghcr_token_local.sh 2>/dev/null || true
	@./scripts/ghcr_token_local.sh

ghcr-login:
	@# Only needed for GHCR/private pulls & pushes
	@./scripts/ghcr_login.sh

images-push:
	@# Ensure docker is logged in for private registry pushes.
	@if echo "$(IMAGE_REPO)" | grep -q "^ghcr.io/"; then $(MAKE) ghcr-login; fi
	@echo "📦 Pushing images (tag=$(IMAGE_TAG))"
	IMAGE_TAG=$(IMAGE_TAG) IMAGE_REPO=$(IMAGE_REPO) \
	  $(COMPOSE) -f $(DEPLOY_COMPOSE_FILE) --env-file $(DEV_ENV_FILE) push api orchestrator finops frontend

images-pull:
	@echo "⬇️  Pulling images (tag=$(IMAGE_TAG))"
	IMAGE_TAG=$(IMAGE_TAG) IMAGE_REPO=$(IMAGE_REPO) \
	  $(COMPOSE) -f $(DEPLOY_COMPOSE_FILE) --env-file $(DEV_ENV_FILE) pull api orchestrator finops frontend

# Optional convenience tags (staging/prod) that point to the same digest as IMAGE_TAG.
images-promote-stg:
	@if echo "$(IMAGE_REPO)" | grep -q "^ghcr.io/"; then $(MAKE) ghcr-login; fi
	@echo "🏷️  Promoting $(IMAGE_TAG) -> staging (same digest)"
	docker buildx imagetools create -t $(IMAGE_REPO)/inventiv-api:staging $(IMAGE_REPO)/inventiv-api:$(IMAGE_TAG)
	docker buildx imagetools create -t $(IMAGE_REPO)/inventiv-orchestrator:staging $(IMAGE_REPO)/inventiv-orchestrator:$(IMAGE_TAG)
	docker buildx imagetools create -t $(IMAGE_REPO)/inventiv-finops:staging $(IMAGE_REPO)/inventiv-finops:$(IMAGE_TAG)
	docker buildx imagetools create -t $(IMAGE_REPO)/inventiv-frontend:staging $(IMAGE_REPO)/inventiv-frontend:$(IMAGE_TAG)

images-promote-prod:
	@if echo "$(IMAGE_REPO)" | grep -q "^ghcr.io/"; then $(MAKE) ghcr-login; fi
	@echo "🏷️  Promoting $(IMAGE_TAG) -> prod (same digest)"
	docker buildx imagetools create -t $(IMAGE_REPO)/inventiv-api:prod $(IMAGE_REPO)/inventiv-api:$(IMAGE_TAG)
	docker buildx imagetools create -t $(IMAGE_REPO)/inventiv-orchestrator:prod $(IMAGE_REPO)/inventiv-orchestrator:$(IMAGE_TAG)
	docker buildx imagetools create -t $(IMAGE_REPO)/inventiv-finops:prod $(IMAGE_REPO)/inventiv-finops:$(IMAGE_TAG)
	docker buildx imagetools create -t $(IMAGE_REPO)/inventiv-frontend:prod $(IMAGE_REPO)/inventiv-frontend:$(IMAGE_TAG)

## -----------------------------
## Local DEV (docker-compose.yml, hot reload)
## -----------------------------

# README-friendly aliases
up: dev-create
down: dev-delete
ps: dev-ps
logs: dev-logs

ui:
	@$(MAKE) check-dev-env
	@echo "🖥️  UI (Docker) on http://localhost:$(UI_HOST_PORT)  [PORT_OFFSET=$(PORT_OFFSET)]"
	@UI_HOST_PORT=$(UI_HOST_PORT) PORT_OFFSET=$(PORT_OFFSET) \
	  $(COMPOSE_LOCAL) --profile ui up -d --remove-orphans frontend

# Optional: run UI locally on host (kept for convenience / debugging).
# Note: requires the API to be exposed on the host (not the default anymore).
.PHONY: ui-local
ui-local:
	@$(MAKE) check-dev-env
	@set -e; \
	if [ ! -d "inventiv-frontend" ]; then \
	  echo "❌ inventiv-frontend/ not found" >&2; exit 2; \
	fi; \
	if [ ! -d "inventiv-frontend/node_modules" ]; then \
	  echo "⚠️  inventiv-frontend/node_modules missing. Run:" >&2; \
	  echo "   cd inventiv-frontend && npm install" >&2; \
	fi; \
	if [ ! -f "inventiv-frontend/.env.local" ]; then \
	  echo "NEXT_PUBLIC_API_URL=http://localhost:8003" > inventiv-frontend/.env.local; \
	  echo "✅ Created inventiv-frontend/.env.local (NEXT_PUBLIC_API_URL=http://localhost:8003)"; \
	fi; \
	cd inventiv-frontend && npm run dev -- --port $(UI_HOST_PORT)

# Expose API on host loopback for local tunnels (cloudflared), without changing docker-compose.yml.
# It starts a tiny socat container bound to 127.0.0.1:(8003+PORT_OFFSET) that forwards to api:8003 on the compose network.
.PHONY: api-expose api-unexpose
api-expose:
	@$(MAKE) check-dev-env
	@chmod +x ./scripts/dev_expose_api_loopback.sh 2>/dev/null || true
	@PORT_OFFSET=$(PORT_OFFSET) ./scripts/dev_expose_api_loopback.sh

api-unexpose:
	@API_HOST_PORT=$$(echo $$((8003 + $${PORT_OFFSET:-0}))); \
	docker rm -f "inventiv-api-loopback-$${API_HOST_PORT}" >/dev/null 2>&1 || true; \
	echo "✅ Removed API loopback proxy (if it existed) on port $${API_HOST_PORT}"

dev-create:
	@echo "🚀 DEV create (docker-compose.yml, hot reload)"
	@$(MAKE) check-dev-env
	$(COMPOSE_LOCAL) up -d --build --remove-orphans

dev-create-edge:
	@$(MAKE) edge-create

dev-start:
	@$(MAKE) check-dev-env
	$(COMPOSE_LOCAL) up -d --remove-orphans

dev-start-edge:
	@$(MAKE) edge-start

dev-stop:
	@$(MAKE) check-dev-env
	$(COMPOSE_LOCAL) stop

dev-delete:
	@$(MAKE) check-dev-env
	$(COMPOSE_LOCAL) down -v

dev-ps:
	@$(MAKE) check-dev-env
	$(COMPOSE_LOCAL) ps

dev-logs:
	@$(MAKE) check-dev-env
	$(COMPOSE_LOCAL) logs -f --tail=200

dev-cert:
	@$(MAKE) edge-cert

# Convenience: when you edit orchestrator code and want to reload the container quickly.
dev-restart-orchestrator:
	@$(MAKE) check-dev-env
	$(COMPOSE_LOCAL) restart orchestrator

reset-admin-password:
	@./scripts/reset_admin_password.sh

## -----------------------------
## Local prod-like stack (deploy/docker-compose.nginx.yml)
## -----------------------------

edge-create:
	@echo "🚀 EDGE create (deploy/docker-compose.nginx.yml) tag=$(IMAGE_TAG)"
	IMAGE_TAG=$(IMAGE_TAG) IMAGE_REPO=$(IMAGE_REPO) \
	  $(COMPOSE_DEPLOY) build
	IMAGE_TAG=$(IMAGE_TAG) IMAGE_REPO=$(IMAGE_REPO) \
	  $(COMPOSE_DEPLOY) up -d --remove-orphans

edge-start:
	IMAGE_TAG=$(IMAGE_TAG) IMAGE_REPO=$(IMAGE_REPO) \
	  $(COMPOSE_DEPLOY) up -d --remove-orphans

edge-stop:
	IMAGE_TAG=$(IMAGE_TAG) IMAGE_REPO=$(IMAGE_REPO) \
	  $(COMPOSE_DEPLOY) stop

edge-delete:
	IMAGE_TAG=$(IMAGE_TAG) IMAGE_REPO=$(IMAGE_REPO) \
	  $(COMPOSE_DEPLOY) down -v

edge-ps:
	IMAGE_TAG=$(IMAGE_TAG) IMAGE_REPO=$(IMAGE_REPO) \
	  $(COMPOSE_DEPLOY) ps

edge-logs:
	IMAGE_TAG=$(IMAGE_TAG) IMAGE_REPO=$(IMAGE_REPO) \
	  $(COMPOSE_DEPLOY) logs -f --tail=200

# Issue wildcard cert (edge profile)
edge-cert:
	IMAGE_TAG=$(IMAGE_TAG) IMAGE_REPO=$(IMAGE_REPO) \
	  $(COMPOSE) -f $(DEPLOY_COMPOSE_FILE) --env-file $(DEV_ENV_FILE) --profile edge run --rm lego

## -----------------------------
## Remote STAGING/PROD (create/update/start/stop/delete)
## -----------------------------

stg-create:
	@$(MAKE) check-stg-env
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	REMOTE=$${STG_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE REMOTE_DIR=$(REMOTE_DIR) ./scripts/deploy_remote.sh staging create

stg-update:
	@$(MAKE) check-stg-env
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	REMOTE=$${STG_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE REMOTE_DIR=$(REMOTE_DIR) ./scripts/deploy_remote.sh staging update

stg-start:
	@$(MAKE) check-stg-env
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	REMOTE=$${STG_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE REMOTE_DIR=$(REMOTE_DIR) ./scripts/deploy_remote.sh staging start

stg-stop:
	@$(MAKE) check-stg-env
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	REMOTE=$${STG_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE REMOTE_DIR=$(REMOTE_DIR) ./scripts/deploy_remote.sh staging stop

stg-delete:
	@$(MAKE) check-stg-env
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	REMOTE=$${STG_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE REMOTE_DIR=$(REMOTE_DIR) ./scripts/deploy_remote.sh staging delete

stg-status:
	@$(MAKE) check-stg-env
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	REMOTE=$${STG_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE REMOTE_DIR=$(REMOTE_DIR) ./scripts/deploy_remote.sh staging status

stg-logs:
	@$(MAKE) check-stg-env
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	REMOTE=$${STG_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE REMOTE_DIR=$(REMOTE_DIR) ./scripts/deploy_remote.sh staging logs

stg-bootstrap:
	@$(MAKE) check-stg-env
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	REMOTE=$${STG_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE REMOTE_DIR=$(REMOTE_DIR) ./scripts/remote_bootstrap.sh staging

stg-provision:
	@$(MAKE) check-stg-env
	@chmod 600 ./.ssh/llm-studio-key 2>/dev/null || true
	@./scripts/scw_instance_provision.sh $(STG_ENV_FILE) staging

stg-ghcr-token:
	@$(MAKE) check-stg-env
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	REMOTE=$${STG_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE ./scripts/remote_set_secret.sh $(STG_ENV_FILE) ghcr_token

stg-secrets-sync:
	@$(MAKE) check-stg-env
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	REMOTE=$${STG_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE ./scripts/remote_sync_secrets.sh $(STG_ENV_FILE)

stg-cert-export:
	@$(MAKE) check-stg-env
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	ROOT=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $$ROOT_DOMAIN'); \
	REMOTE=$${STG_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	mkdir -p deploy/certs; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE ./scripts/lego_volume_export.sh $(STG_ENV_FILE) "deploy/certs/lego_data_$${ROOT}_staging.tar.gz" || true

stg-cert-import:
	@$(MAKE) check-stg-env
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	ROOT=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $$ROOT_DOMAIN'); \
	REMOTE=$${STG_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE ./scripts/lego_volume_import.sh $(STG_ENV_FILE) "deploy/certs/lego_data_$${ROOT}_staging.tar.gz"

stg-rebuild:
	@$(MAKE) check-stg-env
	@$(MAKE) stg-provision
	@$(MAKE) stg-bootstrap
	@$(MAKE) stg-secrets-sync
	@$(MAKE) stg-create

stg-destroy:
	@$(MAKE) check-stg-env
	@./scripts/scw_instance_destroy.sh $(STG_ENV_FILE) staging

stg-cert:
	@$(MAKE) check-stg-env
	REMOTE_SSH=$(STG_REMOTE_SSH) REMOTE_DIR=$(REMOTE_DIR) EDGE_ENABLED=1 \
	  ./scripts/deploy_remote.sh staging cert

stg-renew:
	@$(MAKE) check-stg-env
	REMOTE_SSH=$(STG_REMOTE_SSH) REMOTE_DIR=$(REMOTE_DIR) EDGE_ENABLED=1 \
	  ./scripts/deploy_remote.sh staging renew

prod-create:
	@$(MAKE) check-prod-env
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	REMOTE=$${PRD_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE REMOTE_DIR=$(REMOTE_DIR) ./scripts/deploy_remote.sh prod create

prod-update:
	@$(MAKE) check-prod-env
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	REMOTE=$${PRD_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE REMOTE_DIR=$(REMOTE_DIR) ./scripts/deploy_remote.sh prod update

prod-start:
	@$(MAKE) check-prod-env
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	REMOTE=$${PRD_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE REMOTE_DIR=$(REMOTE_DIR) ./scripts/deploy_remote.sh prod start

prod-stop:
	@$(MAKE) check-prod-env
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	REMOTE=$${PRD_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE REMOTE_DIR=$(REMOTE_DIR) ./scripts/deploy_remote.sh prod stop

prod-delete:
	@$(MAKE) check-prod-env
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	REMOTE=$${PRD_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE REMOTE_DIR=$(REMOTE_DIR) ./scripts/deploy_remote.sh prod delete

prod-status:
	@$(MAKE) check-prod-env
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	REMOTE=$${PRD_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE REMOTE_DIR=$(REMOTE_DIR) ./scripts/deploy_remote.sh prod status

prod-logs:
	@$(MAKE) check-prod-env
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	REMOTE=$${PRD_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE REMOTE_DIR=$(REMOTE_DIR) ./scripts/deploy_remote.sh prod logs

prod-bootstrap:
	@$(MAKE) check-prod-env
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	REMOTE=$${PRD_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE REMOTE_DIR=$(REMOTE_DIR) ./scripts/remote_bootstrap.sh prod

prod-provision:
	@$(MAKE) check-prod-env
	@chmod 600 ./.ssh/llm-studio-key 2>/dev/null || true
	@./scripts/scw_instance_provision.sh $(PRD_ENV_FILE) prod

prod-ghcr-token:
	@$(MAKE) check-prod-env
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	REMOTE=$${PRD_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE ./scripts/remote_set_secret.sh $(PRD_ENV_FILE) ghcr_token

prod-secrets-sync:
	@$(MAKE) check-prod-env
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	REMOTE=$${PRD_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE ./scripts/remote_sync_secrets.sh $(PRD_ENV_FILE)

prod-cert-export:
	@$(MAKE) check-prod-env
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	ROOT=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $$ROOT_DOMAIN'); \
	REMOTE=$${PRD_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	mkdir -p deploy/certs; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE ./scripts/lego_volume_export.sh $(PRD_ENV_FILE) "deploy/certs/lego_data_$${ROOT}_prod.tar.gz" || true

prod-cert-import:
	@$(MAKE) check-prod-env
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	ROOT=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $$ROOT_DOMAIN'); \
	REMOTE=$${PRD_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE ./scripts/lego_volume_import.sh $(PRD_ENV_FILE) "deploy/certs/lego_data_$${ROOT}_prod.tar.gz"

prod-reset-admin-password:
	@$(MAKE) check-prod-env
	@./scripts/reset_admin_password.sh prod

prod-rebuild:
	@$(MAKE) check-prod-env
	@$(MAKE) prod-provision
	@$(MAKE) prod-bootstrap
	@$(MAKE) prod-secrets-sync
	@$(MAKE) prod-create

prod-destroy:
	@$(MAKE) check-prod-env
	@./scripts/scw_instance_destroy.sh $(PRD_ENV_FILE) prod

prod-cert:
	@$(MAKE) check-prod-env
	REMOTE_SSH=$(PRD_REMOTE_SSH) REMOTE_DIR=$(REMOTE_DIR) EDGE_ENABLED=1 \
	  ./scripts/deploy_remote.sh prod cert

prod-renew:
	@$(MAKE) check-prod-env
	REMOTE_SSH=$(PRD_REMOTE_SSH) REMOTE_DIR=$(REMOTE_DIR) EDGE_ENABLED=1 \
	  ./scripts/deploy_remote.sh prod renew

## -----------------------------
## Rust tests / cleanup
## -----------------------------

test:
	cargo test --workspace

clean:
	rm -rf target/
