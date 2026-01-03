VERSION := $(shell cat VERSION)
GIT_SHA := $(shell git rev-parse --short=12 HEAD)

# Port offset (for running multiple worktrees side-by-side on the same host).
# Example:
#   make PORT_OFFSET=0 up ui          -> UI on 3000
#   make PORT_OFFSET=10000 up ui      -> UI on 13000 (no collisions)
PORT_OFFSET ?= 0

# Orchestrator Cargo features (dev/local).
# We compile both Scaleway + Mock so the orchestrator can always operate on instances from either provider
# (otherwise termination/reconciliation can get stuck with "Provider not configured").
ORCHESTRATOR_FEATURES ?= provider-scaleway,provider-mock

# Docker network name used by the control-plane compose project (api/orchestrator/db/redis).
# Per-instance mock runtimes attach to this network so they can reach http://api:8003.
CONTROLPLANE_NETWORK_NAME ?= $(shell echo "$$(basename "$$(pwd)")_default")

# Background external watcher (host process) that auto-attaches mock runtimes for new mock instances.
MOCK_RUNTIME_WATCH_PIDFILE ?= .mock_runtime_watch.pid
MOCK_RUNTIME_WATCH_LOG ?= .mock_runtime_watch.log

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
	docker-prune-old test test-worker-observability test-worker-observability-clean clean check fmt fmt-check clippy \
	ui-install ui-lint ui-build security-check \
	agent-version-bump agent-checksum agent-version-check \
	ci-fast ci

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
	@echo "## Agent version management"
	@echo "  make agent-checksum              # Calculate SHA256 of agent.py"
	@echo "  make agent-version-get           # Show current agent version"
	@echo "  make agent-version-bump [VERSION=1.0.1] [BUILD_DATE=2026-01-03]"
	@echo "  make agent-version-auto-bump    # Auto-increment patch version"
	@echo "  make agent-version-check         # Verify version was updated if agent.py changed"
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
	@echo "  make up | down | ps | logs     # down keeps DB/Redis volumes"
	@echo "  make nuke                       # down -v (wipe DB/Redis volumes)"
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
down: dev-stop
ps: dev-ps
logs: dev-logs

# Destructive reset (removes named volumes like db_data/redis_data)
.PHONY: nuke
nuke: dev-delete

ui:
	@$(MAKE) check-dev-env
	@echo "🖥️  UI (Docker) on http://localhost:$(UI_HOST_PORT)  [PORT_OFFSET=$(PORT_OFFSET)]"
	@ORCHESTRATOR_FEATURES="$(ORCHESTRATOR_FEATURES)" UI_HOST_PORT=$(UI_HOST_PORT) PORT_OFFSET=$(PORT_OFFSET) \
	  $(COMPOSE_LOCAL) --profile ui up -d --remove-orphans frontend

.PHONY: ui-down
ui-down:
	@$(MAKE) check-dev-env
	@echo "🛑 Stopping UI (Docker)  [PORT_OFFSET=$(PORT_OFFSET)]"
	@ORCHESTRATOR_FEATURES="$(ORCHESTRATOR_FEATURES)" UI_HOST_PORT=$(UI_HOST_PORT) PORT_OFFSET=$(PORT_OFFSET) \
	  $(COMPOSE_LOCAL) --profile ui stop frontend >/dev/null 2>&1 || true
	@echo "✅ UI (Docker) stopped"

# Local “mock worker” stack (for observability + OpenAI proxy local tests).
# It starts mock-vllm + worker-agent along with the rest of the stack.
.PHONY: worker-local-up worker-local-down
worker-local-up:
	@echo "🤖 DEV worker-local up (mock-vllm + worker-agent)"
	@$(MAKE) check-dev-env
	@ORCHESTRATOR_FEATURES="$(ORCHESTRATOR_FEATURES)" PORT_OFFSET="$(PORT_OFFSET)" \
	  $(COMPOSE_LOCAL) --profile worker-local up -d --build --remove-orphans

worker-local-down:
	@echo "🤖 DEV worker-local down (mock-vllm + worker-agent)"
	@$(MAKE) check-dev-env
	@ORCHESTRATOR_FEATURES="$(ORCHESTRATOR_FEATURES)" PORT_OFFSET="$(PORT_OFFSET)" \
	  $(COMPOSE_LOCAL) --profile worker-local stop mock-vllm worker-agent >/dev/null 2>&1 || true
	@echo "✅ worker-local stopped"

# Attach the local worker-agent to a specific instance (required for Mock instances to leave BOOTING).
# Usage:
#   make worker-attach INSTANCE_ID=<uuid> [MOCK_VLLM_MODEL_ID=demo-model-...]
.PHONY: worker-attach worker-detach
worker-attach:
	@$(MAKE) check-dev-env
	@if [ -z "$(INSTANCE_ID)" ]; then \
	  echo "❌ INSTANCE_ID is required (example: make worker-attach INSTANCE_ID=<uuid>)"; \
	  exit 2; \
	fi
	@chmod +x ./scripts/mock_runtime_up.sh 2>/dev/null || true
	@MID="$${MOCK_VLLM_MODEL_ID:-demo-model-$$(echo "$(INSTANCE_ID)" | tr -d '-' | cut -c1-12)}"; \
	echo "🔗 Attaching mock runtime to INSTANCE_ID=$(INSTANCE_ID) (MOCK_VLLM_MODEL_ID=$${MID})"; \
	INSTANCE_ID="$(INSTANCE_ID)" MOCK_VLLM_MODEL_ID="$${MID}" CONTROLPLANE_NETWORK_NAME="$(CONTROLPLANE_NETWORK_NAME)" \
	  ./scripts/mock_runtime_up.sh

worker-detach:
	@$(MAKE) check-dev-env
	@chmod +x ./scripts/mock_runtime_down.sh 2>/dev/null || true
	@echo "🔌 Detaching mock runtime for INSTANCE_ID=$(INSTANCE_ID)"
	@INSTANCE_ID="$(INSTANCE_ID)" CONTROLPLANE_NETWORK_NAME="$(CONTROLPLANE_NETWORK_NAME)" \
	  ./scripts/mock_runtime_down.sh
	@echo "✅ worker detached"

.PHONY: mock-runtime-sync
mock-runtime-sync:
	@$(MAKE) check-dev-env
	@chmod +x ./scripts/mock_runtime_sync.sh 2>/dev/null || true
	@CONTROLPLANE_NETWORK_NAME="$(CONTROLPLANE_NETWORK_NAME)" ./scripts/mock_runtime_sync.sh

.PHONY: mock-runtime-watch-up mock-runtime-watch-down
mock-runtime-watch-up:
	@$(MAKE) check-dev-env
	@chmod +x ./scripts/mock_runtime_watch.sh 2>/dev/null || true
	@if [ -f "$(MOCK_RUNTIME_WATCH_PIDFILE)" ] && kill -0 $$(cat "$(MOCK_RUNTIME_WATCH_PIDFILE)" 2>/dev/null) >/dev/null 2>&1; then \
	  echo "👀 mock-runtime-watch already running (pid=$$(cat $(MOCK_RUNTIME_WATCH_PIDFILE)))"; \
	  exit 0; \
	fi
	@rm -f "$(MOCK_RUNTIME_WATCH_PIDFILE)" >/dev/null 2>&1 || true
	@echo "👀 starting mock-runtime-watch (logs: $(MOCK_RUNTIME_WATCH_LOG))"
	@nohup env CONTROLPLANE_NETWORK_NAME="$(CONTROLPLANE_NETWORK_NAME)" WATCH_INTERVAL_S="$${WATCH_INTERVAL_S:-5}" \
	  ./scripts/mock_runtime_watch.sh >"$(MOCK_RUNTIME_WATCH_LOG)" 2>&1 & echo $$! >"$(MOCK_RUNTIME_WATCH_PIDFILE)"
	@echo "✅ mock-runtime-watch started (pid=$$(cat $(MOCK_RUNTIME_WATCH_PIDFILE)))"

mock-runtime-watch-down:
	@$(MAKE) check-dev-env
	@if [ ! -f "$(MOCK_RUNTIME_WATCH_PIDFILE)" ]; then echo "ℹ️  mock-runtime-watch not running"; exit 0; fi
	@PID=$$(cat "$(MOCK_RUNTIME_WATCH_PIDFILE)" 2>/dev/null || true); \
	if [ -n "$$PID" ] && kill -0 $$PID >/dev/null 2>&1; then \
	  echo "🛑 stopping mock-runtime-watch (pid=$$PID)"; \
	  kill $$PID >/dev/null 2>&1 || true; \
	fi
	@rm -f "$(MOCK_RUNTIME_WATCH_PIDFILE)" >/dev/null 2>&1 || true
	@echo "✅ mock-runtime-watch stopped"

# All-in-one local stack (control-plane + UI + local mock worker).
.PHONY: local-up local-down
local-up:
	@echo "🚀 LOCAL up (api+orchestrator+db+redis + ui)  [PORT_OFFSET=$(PORT_OFFSET)]"
	@$(MAKE) check-dev-env
	@ORCHESTRATOR_FEATURES="$(ORCHESTRATOR_FEATURES)" UI_HOST_PORT="$(UI_HOST_PORT)" PORT_OFFSET="$(PORT_OFFSET)" \
	  $(COMPOSE_LOCAL) --profile ui up -d --build --remove-orphans
	@# Mock provider now manages runtimes automatically (no external watcher needed)

local-down:
	@echo "🛑 LOCAL down (stop all local services)  [PORT_OFFSET=$(PORT_OFFSET)]"
	@$(MAKE) check-dev-env
	@ORCHESTRATOR_FEATURES="$(ORCHESTRATOR_FEATURES)" UI_HOST_PORT="$(UI_HOST_PORT)" PORT_OFFSET="$(PORT_OFFSET)" \
	  $(COMPOSE_LOCAL) --profile ui stop >/dev/null 2>&1 || true
	@# Mock provider manages runtimes, but we can do a best-effort cleanup of orphaned runtimes
	@docker ps -a --filter "name=mockrt-" --format "{{.Names}}" | xargs -r docker rm -f >/dev/null 2>&1 || true
	@echo "✅ LOCAL stopped"

# Optional: run UI locally on host (kept for convenience / debugging).
# Note: requires the API to be exposed on the host (not the default anymore).
.PHONY: ui-local
ui-local:
	@$(MAKE) check-dev-env
	@set -e; \
	if [ ! -f "package.json" ]; then \
	  echo "❌ package.json not found at repo root (run from repo root)" >&2; exit 2; \
	fi; \
	echo "🖥️  UI (host) on http://localhost:$(UI_HOST_PORT)  [PORT_OFFSET=$(PORT_OFFSET)]"; \
	echo "ℹ️  This uses npm workspaces. Installing at repo root..."; \
	npm install --no-audit --no-fund; \
	API_HOST_PORT=$$(echo $$((8003 + $(PORT_OFFSET)))); \
	echo "ℹ️  Ensure API is reachable on http://localhost:$${API_HOST_PORT} (tip: make api-expose PORT_OFFSET=$(PORT_OFFSET))"; \
	API_INTERNAL_URL="http://localhost:$${API_HOST_PORT}" \
	  npm -w inventiv-frontend run dev -- --webpack --port $(UI_HOST_PORT)

.PHONY: ui-local-down
ui-local-down:
	@$(MAKE) check-dev-env
	@set -e; \
	PORT="$(UI_HOST_PORT)"; \
	echo "🛑 Stopping UI (host) on port $${PORT}  [PORT_OFFSET=$(PORT_OFFSET)]"; \
	PIDS="$$(lsof -ti tcp:$${PORT} -sTCP:LISTEN 2>/dev/null || true)"; \
	if [ -z "$${PIDS}" ]; then \
	  echo "ℹ️  No process listening on port $${PORT}"; \
	  exit 0; \
	fi; \
	echo "🔪 Killing PID(s): $${PIDS}"; \
	kill $${PIDS} 2>/dev/null || true; \
	sleep 0.2; \
	PIDS2="$$(lsof -ti tcp:$${PORT} -sTCP:LISTEN 2>/dev/null || true)"; \
	if [ -n "$${PIDS2}" ]; then \
	  echo "⚠️  Still listening, force kill: $${PIDS2}"; \
	  kill -9 $${PIDS2} 2>/dev/null || true; \
	fi; \
	echo "✅ UI (host) stopped (or was not running)"

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
	ORCHESTRATOR_FEATURES="$(ORCHESTRATOR_FEATURES)" PORT_OFFSET="$(PORT_OFFSET)" \
	  $(COMPOSE_LOCAL) up -d --build --remove-orphans

dev-create-edge:
	@$(MAKE) edge-create

dev-start:
	@$(MAKE) check-dev-env
	ORCHESTRATOR_FEATURES="$(ORCHESTRATOR_FEATURES)" PORT_OFFSET="$(PORT_OFFSET)" \
	  $(COMPOSE_LOCAL) up -d --remove-orphans

dev-start-edge:
	@$(MAKE) edge-start

dev-stop:
	@$(MAKE) check-dev-env
	$(COMPOSE_LOCAL) stop

dev-delete:
	@$(MAKE) check-dev-env
	@echo "🧹 Cleaning up all inventiv containers..."
	@docker ps -a --filter "name=inventiv" --format "{{.Names}}" | xargs -r docker rm -f >/dev/null 2>&1 || true
	@docker ps -a --filter "name=mockrt-" --format "{{.Names}}" | xargs -r docker rm -f >/dev/null 2>&1 || true
	@docker ps -a --filter "name=inventiv-api-loopback" --format "{{.Names}}" | xargs -r docker rm -f >/dev/null 2>&1 || true
	@echo "🧹 Cleaning up volumes..."
	@docker volume ls --filter "name=mockrt-" --format "{{.Name}}" | xargs -r docker volume rm >/dev/null 2>&1 || true
	@echo "🧹 Stopping compose services..."
	@$(COMPOSE_LOCAL) down -v >/dev/null 2>&1 || true
	@echo "🧹 Cleaning up orphaned networks..."
	@NETWORK_NAME="$(CONTROLPLANE_NETWORK_NAME)"; \
	if docker network inspect "$$NETWORK_NAME" >/dev/null 2>&1; then \
	  docker network inspect "$$NETWORK_NAME" --format '{{range .Containers}}{{.Name}} {{end}}' | \
	  xargs -r docker stop >/dev/null 2>&1 || true; \
	  docker network rm "$$NETWORK_NAME" >/dev/null 2>&1 || true; \
	fi
	@docker network prune -f >/dev/null 2>&1 || true
	@echo "✅ Cleanup complete"
	@echo "🧹 Cleaning up docker-compose services..."
	$(COMPOSE_LOCAL) down -v
	@echo "✅ Cleanup complete"

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

# Integration test (mock provider): API -> Orchestrator -> Worker -> API (observability + OpenAI proxy)
.PHONY: test-worker-observability
test-worker-observability:
	@chmod +x ./scripts/test_worker_observability_mock.sh 2>/dev/null || true
	@RESET_VOLUMES=0 PORT_OFFSET=$(PORT_OFFSET) ./scripts/test_worker_observability_mock.sh

# Prune old/unused Docker resources for this compose project (default: older than 7 days).
# Optional:
# - OLDER_THAN_HOURS=168
# - CLEAN_ALL_UNUSED_IMAGES_OLD=1 (more aggressive; affects other repos)
.PHONY: docker-prune-old
docker-prune-old:
	@chmod +x ./scripts/docker_prune_project_old.sh 2>/dev/null || true
	@COMPOSE_PROJECT_NAME=$$(basename "$$(pwd)") OLDER_THAN_HOURS=$${OLDER_THAN_HOURS:-168} CLEAN_ALL_UNUSED_IMAGES_OLD=$${CLEAN_ALL_UNUSED_IMAGES_OLD:-0} \
	  ./scripts/docker_prune_project_old.sh

# One-shot: prune old docker resources then run the mock observability E2E test.
.PHONY: test-worker-observability-clean
test-worker-observability-clean: docker-prune-old
	@$(MAKE) test-worker-observability PORT_OFFSET=$(PORT_OFFSET)

# Multi-instance integration test (mock provider): serial + parallel create/observability/terminate.
.PHONY: test-worker-observability-multi
test-worker-observability-multi:
	@chmod +x ./scripts/test_worker_observability_mock_multi.sh 2>/dev/null || true
	@PORT_OFFSET=$(PORT_OFFSET) N_SERIAL=$${N_SERIAL:-2} N_PARALLEL=$${N_PARALLEL:-2} \
	  ./scripts/test_worker_observability_mock_multi.sh

# Cargo check (fast compile-only)
check:
	cargo check --workspace

# Rust formatting
fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

# Rust lint (deny warnings)
clippy:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

# Frontend (monorepo) tasks
ui-install:
	npm ci --no-audit --no-fund

ui-lint: ui-install
	npm run lint:ui

ui-build: ui-install
	npm run build:ui

# Security / hygiene checks (tracked files only)
security-check:
	@chmod +x ./scripts/check_no_private_keys.sh 2>/dev/null || true
	@./scripts/check_no_private_keys.sh

# CI presets
ci-fast: security-check fmt-check clippy test ui-lint ui-build
	@echo "✅ ci-fast OK"

# Full local CI (today: same as ci-fast; smoke tests can be added later)
ci: ci-fast
	@echo "✅ ci OK"

clean:
	rm -rf target/

# Agent version management
AGENT_PY := inventiv-worker/agent.py
AGENT_VERSION_REGEX := ^AGENT_VERSION = "([^"]+)"$$
AGENT_BUILD_DATE_REGEX := ^AGENT_BUILD_DATE = "([^"]+)"$$

# Calculate SHA256 checksum of agent.py
.PHONY: agent-checksum
agent-checksum:
	@if [ ! -f "$(AGENT_PY)" ]; then \
		echo "❌ $(AGENT_PY) not found"; \
		exit 1; \
	fi
	@echo "📦 Calculating SHA256 checksum for $(AGENT_PY)..."
	@if command -v sha256sum >/dev/null 2>&1; then \
		sha256sum "$(AGENT_PY)" | cut -d' ' -f1; \
	elif command -v shasum >/dev/null 2>&1; then \
		shasum -a 256 "$(AGENT_PY)" | cut -d' ' -f1; \
	else \
		echo "❌ Neither sha256sum nor shasum found"; \
		exit 1; \
	fi

# Extract current agent version from agent.py
.PHONY: agent-version-get
agent-version-get:
	@if [ ! -f "$(AGENT_PY)" ]; then \
		echo "❌ $(AGENT_PY) not found"; \
		exit 1; \
	fi
	@grep -m1 '^AGENT_VERSION' "$(AGENT_PY)" | sed -E 's/^[^"]*"([^"]+)".*/\1/' || echo "unknown"

# Bump agent version (updates AGENT_VERSION and AGENT_BUILD_DATE)
# Usage: make agent-version-bump [VERSION=1.0.1] [BUILD_DATE=2026-01-03]
.PHONY: agent-version-bump
agent-version-bump:
	@if [ ! -f "$(AGENT_PY)" ]; then \
		echo "❌ $(AGENT_PY) not found"; \
		exit 1; \
	fi
	@CURRENT_VERSION=$$(grep -m1 '^AGENT_VERSION' "$(AGENT_PY)" | sed -E 's/^[^"]*"([^"]+)".*/\1/' || echo ""); \
	CURRENT_DATE=$$(grep -m1 '^AGENT_BUILD_DATE' "$(AGENT_PY)" | sed -E 's/^[^"]*"([^"]+)".*/\1/' || echo ""); \
	NEW_VERSION="$${VERSION:-$$CURRENT_VERSION}"; \
	NEW_DATE="$${BUILD_DATE:-$$(date +%Y-%m-%d)}"; \
	if [ -z "$$NEW_VERSION" ]; then \
		echo "❌ Cannot determine version. Set VERSION=... or ensure AGENT_VERSION exists in $(AGENT_PY)"; \
		exit 1; \
	fi; \
	echo "📝 Updating agent version: $$CURRENT_VERSION -> $$NEW_VERSION"; \
	echo "📅 Build date: $$CURRENT_DATE -> $$NEW_DATE"; \
	if [ "$$CURRENT_VERSION" = "$$NEW_VERSION" ] && [ "$$CURRENT_DATE" = "$$NEW_DATE" ]; then \
		echo "ℹ️  Version and date unchanged, skipping update"; \
		exit 0; \
	fi; \
	if command -v sed >/dev/null 2>&1; then \
		if [ "$$(uname)" = "Darwin" ]; then \
			sed -i '' "s/^AGENT_VERSION = \".*\"/AGENT_VERSION = \"$$NEW_VERSION\"/" "$(AGENT_PY)"; \
			sed -i '' "s/^AGENT_BUILD_DATE = \".*\"/AGENT_BUILD_DATE = \"$$NEW_DATE\"/" "$(AGENT_PY)"; \
		else \
			sed -i "s/^AGENT_VERSION = \".*\"/AGENT_VERSION = \"$$NEW_VERSION\"/" "$(AGENT_PY)"; \
			sed -i "s/^AGENT_BUILD_DATE = \".*\"/AGENT_BUILD_DATE = \"$$NEW_DATE\"/" "$(AGENT_PY)"; \
		fi; \
		echo "✅ Updated $(AGENT_PY)"; \
		echo "   AGENT_VERSION = \"$$NEW_VERSION\""; \
		echo "   AGENT_BUILD_DATE = \"$$NEW_DATE\""; \
		echo ""; \
		echo "📦 New SHA256 checksum:"; \
		$(MAKE) agent-checksum; \
	else \
		echo "❌ sed command not found"; \
		exit 1; \
	fi

# Auto-bump agent version based on git changes (increments patch version)
# Usage: make agent-version-auto-bump
.PHONY: agent-version-auto-bump
agent-version-auto-bump:
	@if [ ! -f "$(AGENT_PY)" ]; then \
		echo "❌ $(AGENT_PY) not found"; \
		exit 1; \
	fi
	@CURRENT_VERSION=$$(grep -m1 '^AGENT_VERSION' "$(AGENT_PY)" | sed -E 's/^[^"]*"([^"]+)".*/\1/' || echo "1.0.0"); \
	MAJOR=$$(echo "$$CURRENT_VERSION" | cut -d. -f1); \
	MINOR=$$(echo "$$CURRENT_VERSION" | cut -d. -f2); \
	PATCH=$$(echo "$$CURRENT_VERSION" | cut -d. -f3); \
	if [ -z "$$PATCH" ]; then PATCH=0; fi; \
	NEW_PATCH=$$((PATCH + 1)); \
	NEW_VERSION="$$MAJOR.$$MINOR.$$NEW_PATCH"; \
	echo "🔄 Auto-bumping version: $$CURRENT_VERSION -> $$NEW_VERSION"; \
	$(MAKE) agent-version-bump VERSION="$$NEW_VERSION"

# Check if agent.py version needs to be updated (for CI)
# Fails if agent.py was modified but version wasn't bumped
.PHONY: agent-version-check
agent-version-check:
	@if [ ! -f "$(AGENT_PY)" ]; then \
		echo "❌ $(AGENT_PY) not found"; \
		exit 1; \
	fi
	@echo "🔍 Checking if agent.py version is up-to-date..."
	@if ! git diff --quiet HEAD -- "$(AGENT_PY)" 2>/dev/null; then \
		echo "⚠️  $(AGENT_PY) has uncommitted changes"; \
		git diff HEAD -- "$(AGENT_PY)" | head -20; \
		echo ""; \
		echo "💡 Run 'make agent-version-auto-bump' to update version automatically"; \
		exit 1; \
	fi; \
	if git diff --quiet HEAD~1 HEAD -- "$(AGENT_PY)" 2>/dev/null; then \
		echo "✅ $(AGENT_PY) unchanged in last commit"; \
		exit 0; \
	fi; \
	CURRENT_VERSION=$$(grep -m1 '^AGENT_VERSION' "$(AGENT_PY)" | sed -E 's/^[^"]*"([^"]+)".*/\1/' || echo ""); \
	PREV_VERSION=$$(git show HEAD~1:"$(AGENT_PY)" 2>/dev/null | grep -m1 '^AGENT_VERSION' | sed -E 's/^[^"]*"([^"]+)".*/\1/' || echo ""); \
	if [ -z "$$CURRENT_VERSION" ]; then \
		echo "❌ AGENT_VERSION not found in $(AGENT_PY)"; \
		exit 1; \
	fi; \
	if [ "$$CURRENT_VERSION" = "$$PREV_VERSION" ]; then \
		echo "❌ $(AGENT_PY) was modified but AGENT_VERSION wasn't updated"; \
		echo "   Current version: $$CURRENT_VERSION"; \
		echo "   Previous version: $$PREV_VERSION"; \
		echo ""; \
		echo "💡 Run 'make agent-version-auto-bump' to update version automatically"; \
		exit 1; \
	fi; \
	echo "✅ Version updated: $$PREV_VERSION -> $$CURRENT_VERSION"; \
	CURRENT_DATE=$$(grep -m1 '^AGENT_BUILD_DATE' "$(AGENT_PY)" | sed -E 's/^[^"]*"([^"]+)".*/\1/' || echo ""); \
	echo "   Build date: $$CURRENT_DATE"
