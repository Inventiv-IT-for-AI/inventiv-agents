VERSION := $(shell cat VERSION)
GIT_SHA := $(shell git rev-parse --short=12 HEAD)

# Container promotion (immutable tags)
IMAGE_TAG ?= $(GIT_SHA)
# Convenience tags (opt-in)
IMAGE_TAG_VERSION ?= v$(VERSION)
IMAGE_TAG_LATEST ?= latest
# Docker requires lowercase repository names
IMAGE_REPO ?= ghcr.io/inventiv-it-for-ai/inventiv-agents

# Compose (single file for build+pull)
COMPOSE_FILE ?= deploy/docker-compose.nginx.yml
COMPOSE ?= docker compose

# Local env files (not committed) should live in env/*.env
# Defaults to *.env.example so commands remain runnable on fresh clones.
DEV_ENV_FILE ?= env/dev.env.example
STG_ENV_FILE ?= env/staging.env
PRD_ENV_FILE ?= env/prod.env.example

# Remote deploy
REMOTE_DIR ?= /opt/inventiv-agents
STG_REMOTE_SSH ?=
PRD_REMOTE_SSH ?=

.PHONY: help \
	images-build images-push images-pull images-promote-stg images-promote-prod \
	images-build-version images-push-version images-build-latest images-push-latest \
	images-publish-stg images-publish-prod \
	ghcr-token-local ghcr-login \
	dev-create dev-create-edge dev-start dev-start-edge dev-stop dev-delete dev-ps dev-logs dev-cert \
	stg-provision stg-destroy stg-bootstrap stg-secrets-sync stg-rebuild stg-create stg-update stg-start stg-stop stg-delete stg-status stg-logs stg-cert stg-renew stg-ghcr-token \
	prod-provision prod-destroy prod-bootstrap prod-secrets-sync prod-rebuild prod-create prod-update prod-start prod-stop prod-delete prod-status prod-logs prod-cert prod-renew prod-ghcr-token \
	test clean

help:
	@echo ""
	@echo "Inventiv-Agents tooling (container promotion)"
	@echo ""
	@echo "Common vars:"
	@echo "  IMAGE_REPO=$(IMAGE_REPO)"
	@echo "  IMAGE_TAG (default)=$(IMAGE_TAG)   | version tag=$(IMAGE_TAG_VERSION)   | latest tag=$(IMAGE_TAG_LATEST)"
	@echo "  COMPOSE_FILE=$(COMPOSE_FILE)"
	@echo ""
	@echo "## Images (build/push/pull)"
	@echo "  make images-build [IMAGE_TAG=<sha>]"
	@echo "  make images-push  [IMAGE_TAG=<sha>]"
	@echo "  make images-pull  [IMAGE_TAG=<sha>]"
	@echo "  make images-build-version    # IMAGE_TAG=v$(VERSION)"
	@echo "  make images-push-version     # IMAGE_TAG=v$(VERSION)"
	@echo "  make images-build-latest     # IMAGE_TAG=latest (dev only)"
	@echo "  make images-push-latest      # IMAGE_TAG=latest (dev only)"
	@echo "  make ghcr-token-local        # writes deploy/secrets/ghcr_token (gitignored) from gh auth token"
	@echo ""
	@echo "## Promotion (same digest)"
	@echo "  make images-promote-stg  IMAGE_TAG=<sha|vX.Y.Z>"
	@echo "  make images-promote-prod IMAGE_TAG=<sha|vX.Y.Z>"
	@echo "  make images-publish-stg  # build+push v$(VERSION) then retag to :staging"
	@echo "  make images-publish-prod # build+push v$(VERSION) then retag to :prod"
	@echo ""
	@echo "## DEV local (docker compose)"
	@echo "  make dev-create        [IMAGE_TAG=<sha|dev|latest>]"
	@echo "  make dev-create-edge   [IMAGE_TAG=<sha|dev|latest>]   # nginx+lego profile edge"
	@echo "  make dev-start | dev-start-edge | dev-stop | dev-delete"
	@echo "  make dev-ps | dev-logs | dev-cert"
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
	@echo ""
	@echo "## PROD remote (Scaleway)"
	@echo "  make prod-provision | prod-destroy | prod-bootstrap | prod-secrets-sync | prod-rebuild | prod-create | prod-update"
	@echo "  make prod-status | prod-logs | prod-stop | prod-delete"
	@echo "  make prod-cert | prod-renew"
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
	  $(COMPOSE) -f $(COMPOSE_FILE) --env-file $(DEV_ENV_FILE) build api orchestrator finops frontend

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
	@mkdir -p deploy/secrets
	@echo "==> Writing deploy/secrets/ghcr_token (gitignored)"
	@# Non-interactive: prefer GHCR_TOKEN if provided, otherwise use gh auth token (may lack read:packages).
	@if [ -n "$${GHCR_TOKEN:-}" ]; then \
	  printf "%s" "$${GHCR_TOKEN}" > deploy/secrets/ghcr_token; \
	else \
	  gh auth token > deploy/secrets/ghcr_token; \
	fi
	@chmod 600 deploy/secrets/ghcr_token
	@echo "==> Note: token must have read:packages for private GHCR pulls"

ghcr-login:
	@# Only needed for GHCR/private pulls & pushes
	@./scripts/ghcr_login.sh

images-push:
	@# Ensure docker is logged in for private registry pushes.
	@if echo "$(IMAGE_REPO)" | grep -q "^ghcr.io/"; then $(MAKE) ghcr-login; fi
	@echo "📦 Pushing images (tag=$(IMAGE_TAG))"
	IMAGE_TAG=$(IMAGE_TAG) IMAGE_REPO=$(IMAGE_REPO) \
	  $(COMPOSE) -f $(COMPOSE_FILE) --env-file $(DEV_ENV_FILE) push api orchestrator finops frontend

images-pull:
	@echo "⬇️  Pulling images (tag=$(IMAGE_TAG))"
	IMAGE_TAG=$(IMAGE_TAG) IMAGE_REPO=$(IMAGE_REPO) \
	  $(COMPOSE) -f $(COMPOSE_FILE) --env-file $(DEV_ENV_FILE) pull api orchestrator finops frontend

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
## Local DEV (create/start/stop/delete)
## -----------------------------

dev-create:
	@echo "🚀 DEV create (no edge) tag=$(IMAGE_TAG)"
	IMAGE_TAG=$(IMAGE_TAG) IMAGE_REPO=$(IMAGE_REPO) \
	  $(COMPOSE) -f $(COMPOSE_FILE) --env-file $(DEV_ENV_FILE) build
	IMAGE_TAG=$(IMAGE_TAG) IMAGE_REPO=$(IMAGE_REPO) \
	  $(COMPOSE) -f $(COMPOSE_FILE) --env-file $(DEV_ENV_FILE) up -d --remove-orphans

dev-create-edge:
	@echo "🚀 DEV create (with edge) tag=$(IMAGE_TAG)"
	IMAGE_TAG=$(IMAGE_TAG) IMAGE_REPO=$(IMAGE_REPO) \
	  $(COMPOSE) -f $(COMPOSE_FILE) --env-file $(DEV_ENV_FILE) --profile edge build
	IMAGE_TAG=$(IMAGE_TAG) IMAGE_REPO=$(IMAGE_REPO) \
	  $(COMPOSE) -f $(COMPOSE_FILE) --env-file $(DEV_ENV_FILE) --profile edge up -d --remove-orphans

dev-start:
	IMAGE_TAG=$(IMAGE_TAG) IMAGE_REPO=$(IMAGE_REPO) \
	  $(COMPOSE) -f $(COMPOSE_FILE) --env-file $(DEV_ENV_FILE) up -d --remove-orphans

dev-start-edge:
	IMAGE_TAG=$(IMAGE_TAG) IMAGE_REPO=$(IMAGE_REPO) \
	  $(COMPOSE) -f $(COMPOSE_FILE) --env-file $(DEV_ENV_FILE) --profile edge up -d --remove-orphans

dev-stop:
	IMAGE_TAG=$(IMAGE_TAG) IMAGE_REPO=$(IMAGE_REPO) \
	  $(COMPOSE) -f $(COMPOSE_FILE) --env-file $(DEV_ENV_FILE) stop

dev-delete:
	IMAGE_TAG=$(IMAGE_TAG) IMAGE_REPO=$(IMAGE_REPO) \
	  $(COMPOSE) -f $(COMPOSE_FILE) --env-file $(DEV_ENV_FILE) down -v

dev-ps:
	IMAGE_TAG=$(IMAGE_TAG) IMAGE_REPO=$(IMAGE_REPO) \
	  $(COMPOSE) -f $(COMPOSE_FILE) --env-file $(DEV_ENV_FILE) ps

dev-logs:
	IMAGE_TAG=$(IMAGE_TAG) IMAGE_REPO=$(IMAGE_REPO) \
	  $(COMPOSE) -f $(COMPOSE_FILE) --env-file $(DEV_ENV_FILE) logs -f --tail=200

# Issue wildcard cert (edge profile)
dev-cert:
	IMAGE_TAG=$(IMAGE_TAG) IMAGE_REPO=$(IMAGE_REPO) \
	  $(COMPOSE) -f $(COMPOSE_FILE) --env-file $(DEV_ENV_FILE) --profile edge run --rm lego

## -----------------------------
## Remote STAGING/PROD (create/update/start/stop/delete)
## -----------------------------

stg-create:
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	REMOTE=$${STG_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE REMOTE_DIR=$(REMOTE_DIR) ./scripts/deploy_remote.sh staging create

stg-update:
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	REMOTE=$${STG_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE REMOTE_DIR=$(REMOTE_DIR) ./scripts/deploy_remote.sh staging update

stg-start:
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	REMOTE=$${STG_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE REMOTE_DIR=$(REMOTE_DIR) ./scripts/deploy_remote.sh staging start

stg-stop:
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	REMOTE=$${STG_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE REMOTE_DIR=$(REMOTE_DIR) ./scripts/deploy_remote.sh staging stop

stg-delete:
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	REMOTE=$${STG_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE REMOTE_DIR=$(REMOTE_DIR) ./scripts/deploy_remote.sh staging delete

stg-status:
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	REMOTE=$${STG_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE REMOTE_DIR=$(REMOTE_DIR) ./scripts/deploy_remote.sh staging status

stg-logs:
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	REMOTE=$${STG_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE REMOTE_DIR=$(REMOTE_DIR) ./scripts/deploy_remote.sh staging logs

stg-bootstrap:
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	REMOTE=$${STG_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE REMOTE_DIR=$(REMOTE_DIR) ./scripts/remote_bootstrap.sh staging

stg-provision:
	@chmod 600 ./.ssh/llm-studio-key 2>/dev/null || true
	@./scripts/scw_instance_provision.sh $(STG_ENV_FILE) staging

stg-ghcr-token:
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	REMOTE=$${STG_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE ./scripts/remote_set_secret.sh $(STG_ENV_FILE) ghcr_token

stg-secrets-sync:
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(STG_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	REMOTE=$${STG_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE ./scripts/remote_sync_secrets.sh $(STG_ENV_FILE)

stg-rebuild:
	@$(MAKE) stg-provision
	@$(MAKE) stg-bootstrap
	@$(MAKE) stg-secrets-sync
	@$(MAKE) stg-create

stg-destroy:
	@./scripts/scw_instance_destroy.sh $(STG_ENV_FILE) staging

stg-cert:
	REMOTE_SSH=$(STG_REMOTE_SSH) REMOTE_DIR=$(REMOTE_DIR) EDGE_ENABLED=1 \
	  ./scripts/deploy_remote.sh staging cert

stg-renew:
	REMOTE_SSH=$(STG_REMOTE_SSH) REMOTE_DIR=$(REMOTE_DIR) EDGE_ENABLED=1 \
	  ./scripts/deploy_remote.sh staging renew

prod-create:
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	REMOTE=$${PRD_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE REMOTE_DIR=$(REMOTE_DIR) ./scripts/deploy_remote.sh prod create

prod-update:
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	REMOTE=$${PRD_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE REMOTE_DIR=$(REMOTE_DIR) ./scripts/deploy_remote.sh prod update

prod-start:
	REMOTE_SSH=$(PRD_REMOTE_SSH) REMOTE_DIR=$(REMOTE_DIR) \
	  ./scripts/deploy_remote.sh prod start

prod-stop:
	REMOTE_SSH=$(PRD_REMOTE_SSH) REMOTE_DIR=$(REMOTE_DIR) \
	  ./scripts/deploy_remote.sh prod stop

prod-delete:
	REMOTE_SSH=$(PRD_REMOTE_SSH) REMOTE_DIR=$(REMOTE_DIR) \
	  ./scripts/deploy_remote.sh prod delete

prod-status:
	REMOTE_SSH=$(PRD_REMOTE_SSH) REMOTE_DIR=$(REMOTE_DIR) \
	  ./scripts/deploy_remote.sh prod status

prod-logs:
	REMOTE_SSH=$(PRD_REMOTE_SSH) REMOTE_DIR=$(REMOTE_DIR) \
	  ./scripts/deploy_remote.sh prod logs

prod-bootstrap:
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	REMOTE=$${PRD_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE REMOTE_DIR=$(REMOTE_DIR) ./scripts/remote_bootstrap.sh prod

prod-provision:
	@chmod 600 ./.ssh/llm-studio-key 2>/dev/null || true
	@./scripts/scw_instance_provision.sh $(PRD_ENV_FILE) prod

prod-ghcr-token:
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	REMOTE=$${PRD_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE ./scripts/remote_set_secret.sh $(PRD_ENV_FILE) ghcr_token

prod-secrets-sync:
	@set -e; \
	HOST=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $$REMOTE_HOST'); \
	PORT=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${REMOTE_PORT:-22}'); \
	USER=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${REMOTE_USER:-}'); \
	KEY=$$(bash -lc 'set -a; source $(PRD_ENV_FILE); set +a; echo $${SSH_IDENTITY_FILE:-}'); \
	REMOTE=$${PRD_REMOTE_SSH:-$${USER:+$$USER@$$HOST}}; \
	if [ -z "$$REMOTE" ]; then REMOTE=$$(SSH_IDENTITY_FILE="$$KEY" ./scripts/ssh_detect_user.sh $$HOST $$PORT); fi; \
	SSH_IDENTITY_FILE="$$KEY" REMOTE_SSH=$$REMOTE ./scripts/remote_sync_secrets.sh $(PRD_ENV_FILE)

prod-rebuild:
	@$(MAKE) prod-provision
	@$(MAKE) prod-bootstrap
	@$(MAKE) prod-secrets-sync
	@$(MAKE) prod-create

prod-destroy:
	@./scripts/scw_instance_destroy.sh $(PRD_ENV_FILE) prod

prod-cert:
	REMOTE_SSH=$(PRD_REMOTE_SSH) REMOTE_DIR=$(REMOTE_DIR) EDGE_ENABLED=1 \
	  ./scripts/deploy_remote.sh prod cert

prod-renew:
	REMOTE_SSH=$(PRD_REMOTE_SSH) REMOTE_DIR=$(REMOTE_DIR) EDGE_ENABLED=1 \
	  ./scripts/deploy_remote.sh prod renew

## -----------------------------
## Rust tests / cleanup
## -----------------------------

test:
	cargo test --workspace

clean:
	rm -rf target/
