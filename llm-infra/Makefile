VERSION := $(shell cat VERSION)
COMPOSE_FILE := docker-compose.yml
PROJECT_NAME := llm-infra

.PHONY: all build up down logs clean test context-dev context-stg context-prod

# --- General ---
all: build

build:
	@echo "🏗  Building services (Version $(VERSION))..."
	docker-compose build

up:
	@echo "🚀 Starting services..."
	docker-compose up -d

down:
	@echo "🛑 Stopping services..."
	docker-compose down

logs:
	docker-compose logs -f

clean:
	@echo "🧹 Cleaning up..."
	docker-compose down -v
	rm -rf target/

test:
	@echo "🧪 Running Tests..."
	# Run unit tests inside docker or local cargo
	cargo test --workspace

# --- Environments ---

# DEV: Local environment (Default)
env-dev:
	@echo "Environment: DEV (Mock Provider)"
	@echo "PROVIDER=mock" > .env
	@echo "RUST_LOG=debug" >> .env

# STAGING: Scaleway Staging
env-stg:
	@echo "Environment: STAGING (Scaleway)"
	@echo "PROVIDER=scaleway" > .env
	@echo "SCALEWAY_PROJECT_ID=${SCALEWAY_STG_PROJECT_ID}" >> .env
	@echo "RUST_LOG=info" >> .env

# PROD: Scaleway Production
env-prod:
	@echo "Environment: PRODUCTION (Scaleway)"
	@echo "PROVIDER=scaleway" > .env
	@echo "SCALEWAY_PROJECT_ID=${SCALEWAY_PRD_PROJECT_ID}" >> .env
	@echo "RUST_LOG=warn" >> .env

# --- Deployment Wrappers ---
deploy-stg: env-stg build up
	@echo "✅ Deployed to Staging"

deploy-prod: env-prod build up
	@echo "🚀 Deployed to Production"
