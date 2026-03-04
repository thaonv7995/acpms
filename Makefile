# ACPMS - Agentic Coding Project Management System
# Single entry point for dev, build, deploy. Run: make help

.PHONY: help setup dev infra-up infra-down migrate build test health clean deploy

# Detect docker compose command
COMPOSE := $(shell docker compose version >/dev/null 2>&1 && echo "docker compose" || echo "docker-compose")

# Default target
help:
	@echo "ACPMS - Makefile"
	@echo ""
	@echo "Development:"
	@echo "  make setup      - First-time setup (.env, deps)"
	@echo "  make dev        - Start dev (infra + backend + frontend)"
	@echo "  make infra-up   - Start PostgreSQL + MinIO"
	@echo "  make infra-down - Stop Docker services"
	@echo "  make migrate    - Run database migrations"
	@echo ""
	@echo "Build & Test:"
	@echo "  make build     - Build backend + frontend for production"
	@echo "  make test      - Run all tests"
	@echo "  make health    - Health check all services"
	@echo ""
	@echo "Production Deploy:"
	@echo "  make deploy    - Build + migrate (see DEPLOY.md)"
	@echo ""
	@echo "Other:"
	@echo "  make clean     - Remove build artifacts"

# --- Setup ---
setup:
	@[ -f .env ] || (cp .env.example .env && echo "Created .env - edit DATABASE_URL (use localhost when running from host)")
	@[ -f frontend/.env.local ] || (echo "VITE_API_URL=http://localhost:3000" > frontend/.env.local && echo "Created frontend/.env.local")
	@cd frontend && npm install
	@echo "Setup done. Run: make infra-up && make migrate && make dev"

# --- Development ---
dev:
	@./scripts/dev.sh

infra-up:
	@$(COMPOSE) up -d postgres minio
	@echo "Waiting for services..."
	@sleep 5
	@$(COMPOSE) ps

infra-down:
	@$(COMPOSE) down

migrate:
	@(command -v sqlx >/dev/null || cargo install sqlx-cli --no-default-features --features postgres)
	@cd crates/db && sqlx migrate run

# --- Build ---
build: build-backend build-frontend

build-backend:
	@cargo build --release --bin acpms-server
	@echo "Backend: target/release/acpms-server"

build-frontend:
	@cd frontend && npm ci --omit=dev && npm run build
	@echo "Frontend: frontend/dist/"

# --- Test ---
test:
	@cargo test --workspace
	@cd frontend && npm run test -- --run

# --- Health ---
health:
	@./scripts/health-check.sh

# --- Clean ---
clean:
	@cargo clean
	@rm -rf frontend/node_modules frontend/dist
	@$(COMPOSE) down -v 2>/dev/null || true
	@echo "Cleaned"

# --- Production Deploy ---
# Steps: 1) build, 2) migrate (DATABASE_URL from .env), 3) start services
deploy: build migrate
	@echo ""
	@echo "Deploy complete. Next steps:"
	@echo "  1. Start backend: ./target/release/acpms-server (or use systemd/docker)"
	@echo "  2. Serve frontend: cd frontend && npx serve dist (or nginx/Cloudflare Pages)"
	@echo "  3. Ensure DATABASE_URL, JWT_SECRET, ENCRYPTION_KEY in .env"
