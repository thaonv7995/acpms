# ACPMS Deployment Guide

Deploy ACPMS via **Makefile** – run `make help` to list commands.

## Quick Start (Development)

```bash
make setup          # First time: create .env, install deps
make infra-up      # Start PostgreSQL + MinIO
make migrate       # Run migrations (DATABASE_URL must use localhost)
make dev           # Backend + Frontend (Ctrl+C to stop)
```

**Note:** In `.env`, change `@postgres` to `@localhost` in `DATABASE_URL` when running backend/migrate from host.

## Production Deploy

```bash
# 1. Configure
cp .env.example .env
# Edit .env: DATABASE_URL, JWT_SECRET, ENCRYPTION_KEY (openssl rand -base64 32)
# Optional: WORKTREES_PATH=${HOME}/Projects — directory for agent-cloned source (shown in Settings)

# 2. Build + migrate
make deploy

# 3. Run backend
./target/release/acpms-server

# 4. Serve frontend (choose one)
# Option A: npx serve frontend/dist
# Option B: Copy frontend/dist to Nginx/Caddy
# Option C: Cloudflare Pages: npx wrangler pages deploy frontend/dist
```

## Make Commands

| Command | Description |
|---------|-------------|
| `make setup` | Initial setup |
| `make dev` | Run dev (infra + backend + frontend) |
| `make infra-up` | Start PostgreSQL + MinIO |
| `make infra-down` | Stop Docker |
| `make migrate` | Run DB migrations |
| `make build` | Build production |
| `make test` | Run tests |
| `make health` | Check services |
| `make deploy` | Build + migrate (production) |
| `make clean` | Remove build artifacts |

## Requirements

- Docker, Rust, Node.js 20+
- `.env` from `.env.example`
