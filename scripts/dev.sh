#!/usr/bin/env bash
# Run ACPMS development stack (backend + frontend) locally.
# If required Docker services are not running, start them first.
#
# Usage:
#   ./scripts/dev.sh           # Default: backend + Vite dev (HMR)
#   ./scripts/dev.sh --single  # Single binary: build frontend, backend serves static (like production)
#   ./scripts/dev.sh --parity  # Production-parity backend runtime (release binary + restricted env)
#
# Optional dev bootstrap env:
#   ACPMS_DEV_SEED_ADMIN=1                # default 1; ensure a dev admin exists before starting servers
#   ACPMS_DEV_ADMIN_EMAIL=admin@acpms.local
#   ACPMS_DEV_ADMIN_PASSWORD=acpms-dev-admin-123

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BACKEND_DIR="$PROJECT_ROOT/crates/server"
FRONTEND_DIR="$PROJECT_ROOT/frontend"
FRONTEND_DIST="$FRONTEND_DIR/dist"
SKILLS_DIR="$PROJECT_ROOT/.acpms/skills"
BACKEND_RELEASE_BIN="$PROJECT_ROOT/target/release/acpms-server"
MAPPED_PG_PORT=""

SINGLE_MODE=0
PARITY_MODE=0
PARITY_PATH_OVERRIDE=""
for arg in "$@"; do
    case "$arg" in
        --single|--binary) SINGLE_MODE=1 ;;
        --parity) PARITY_MODE=1; SINGLE_MODE=1 ;;
        --parity-path=*) PARITY_PATH_OVERRIDE="${arg#*=}" ;;
        -h|--help)
            echo "Usage: $0 [--single|--binary|--parity|--parity-path=<PATH>]"
            echo "  Default: backend + Vite dev server (HMR)"
            echo "  --single: build frontend, backend serves static (single binary mode)"
            echo "  --parity: single binary mode with production-like runtime env (release backend, APP_ENV=production)"
            echo "  --parity-path: override PATH used by parity mode (default mirrors install.sh systemd PATH)"
            exit 0
            ;;
    esac
done

BLUE='\033[0;34m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

load_env_file() {
    local env_file="$1"
    [ -f "$env_file" ] || return 0

    print_info "Loading environment from ${env_file#$PROJECT_ROOT/}"
    set -a
    # shellcheck source=/dev/null
    . "$env_file"
    set +a
}

load_env_file "$PROJECT_ROOT/.env"
load_env_file "$PROJECT_ROOT/.env.local"

require_command() {
    local command_name="$1"
    local install_hint="$2"

    if ! command -v "$command_name" >/dev/null 2>&1; then
        print_error "Missing command: $command_name"
        print_error "Install: $install_hint"
        exit 1
    fi
}

get_mapped_postgres_port() {
    docker port acpms-postgres 5432 2>/dev/null | awk -F: 'NR == 1 {print $NF; exit}'
}

postgres_container_uses_expected_port_binding() {
    docker port acpms-postgres 5432 2>/dev/null | grep -q '^127\.0\.0\.1:'
}

prepare_postgres_runtime() {
    local retries=0

    while [ "$retries" -lt 10 ]; do
        MAPPED_PG_PORT="$(get_mapped_postgres_port || true)"
        if [ -n "$MAPPED_PG_PORT" ]; then
            export DATABASE_URL="postgres://acpms_user:acpms_password@127.0.0.1:${MAPPED_PG_PORT}/acpms"
            return 0
        fi
        retries=$((retries + 1))
        sleep 1
    done

    print_error "Could not determine the published PostgreSQL port"
    return 1
}

detect_compose_cmd() {
    if docker compose version >/dev/null 2>&1; then
        COMPOSE_CMD=(docker compose)
    elif command -v docker-compose >/dev/null 2>&1; then
        COMPOSE_CMD=(docker-compose)
    else
        print_error "Docker Compose is not installed"
        print_error "Install Docker Desktop or docker-compose"
        exit 1
    fi
}

# Map service name to container name (from docker-compose.yml)
service_container_name() {
    case "$1" in
        postgres) echo "acpms-postgres" ;;
        minio)    echo "acpms-minio" ;;
        *)        echo "" ;;
    esac
}

is_service_running() {
    local service="$1"
    local container_id
    local container_name
    local status

    container_id="$("${COMPOSE_CMD[@]}" -f "$PROJECT_ROOT/docker-compose.yml" ps -q "$service" 2>/dev/null || true)"
    if [ -n "$container_id" ]; then
        status="$(docker inspect -f '{{.State.Status}}' "$container_id" 2>/dev/null || true)"
        [ "$status" = "running" ]
        return
    fi

    # Fallback: container may exist but not be managed by this compose (e.g. from previous run)
    container_name="$(service_container_name "$service")"
    if [ -n "$container_name" ] && docker inspect "$container_name" >/dev/null 2>&1; then
        status="$(docker inspect -f '{{.State.Status}}' "$container_name" 2>/dev/null || true)"
        [ "$status" = "running" ]
        return
    fi
    return 1
}

# Remove existing containers with our names if they are not running (avoids "name already in use")
remove_stale_containers() {
    local names=(acpms-postgres acpms-minio)
    local name status
    for name in "${names[@]}"; do
        if docker inspect "$name" >/dev/null 2>&1; then
            status="$(docker inspect -f '{{.State.Status}}' "$name" 2>/dev/null || true)"
            if [ "$status" != "running" ]; then
                print_info "Removing stale container: $name (status=$status)"
                docker rm -f "$name" >/dev/null 2>&1 || true
            fi
        fi
    done
}

ensure_infra_running() {
    local needs_start=0

    if docker inspect acpms-postgres >/dev/null 2>&1 && ! postgres_container_uses_expected_port_binding; then
        print_warning "PostgreSQL container is using a stale port mapping; recreating it"
        docker rm -f acpms-postgres >/dev/null 2>&1 || true
    fi

    if ! is_service_running "postgres"; then
        print_warning "PostgreSQL is not running"
        needs_start=1
    fi

    if ! is_service_running "minio"; then
        print_warning "MinIO is not running"
        needs_start=1
    fi

    if [ "$needs_start" -eq 1 ]; then
        remove_stale_containers
        print_info "Starting Docker services: postgres + minio"
        "${COMPOSE_CMD[@]}" -f "$PROJECT_ROOT/docker-compose.yml" up -d postgres minio
        print_success "Docker services started"
    else
        print_success "Docker services already running (postgres + minio)"
    fi
}

wait_for_service_ready() {
    local service="$1"
    local timeout_seconds="${2:-60}"
    local elapsed=0
    local container_id=""
    local container_name=""
    local status=""
    local health=""

    container_id="$("${COMPOSE_CMD[@]}" -f "$PROJECT_ROOT/docker-compose.yml" ps -q "$service" 2>/dev/null || true)"
    if [ -z "$container_id" ]; then
        container_name="$(service_container_name "$service")"
        if [ -n "$container_name" ] && docker inspect "$container_name" >/dev/null 2>&1; then
            container_id="$(docker inspect -f '{{.Id}}' "$container_name" 2>/dev/null || true)"
        fi
    fi
    if [ -z "$container_id" ]; then
        print_error "Could not find container for service: $service"
        return 1
    fi

    print_info "Waiting for $service to be ready..."
    while [ "$elapsed" -lt "$timeout_seconds" ]; do
        status="$(docker inspect -f '{{.State.Status}}' "$container_id" 2>/dev/null || true)"
        health="$(docker inspect -f '{{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}}' "$container_id" 2>/dev/null || true)"

        if [ "$status" = "running" ] && { [ "$health" = "healthy" ] || [ "$health" = "none" ]; }; then
            print_success "$service is ready"
            return 0
        fi

        sleep 1
        elapsed=$((elapsed + 1))
    done

    print_error "$service is not ready after ${timeout_seconds}s (status=$status, health=$health)"
    return 1
}

ensure_frontend_deps() {
    if [ ! -d "$FRONTEND_DIR/node_modules" ]; then
        print_info "Installing frontend dependencies..."
        (cd "$FRONTEND_DIR" && npm install)
        print_success "Frontend dependencies installed"
    fi
}

build_frontend_for_single_mode() {
    ensure_frontend_deps
    print_info "Building frontend for single binary mode..."
    (cd "$FRONTEND_DIR" && npm run build)
    if [ ! -f "$FRONTEND_DIST/index.html" ]; then
        print_error "Frontend build failed: index.html not found"
        exit 1
    fi
    print_success "Frontend built at $FRONTEND_DIST"
}

build_backend_release_binary() {
    print_info "Building backend release binary..."
    (cd "$PROJECT_ROOT" && cargo build -p acpms-server --release --bin acpms-server)
    if [ ! -x "$BACKEND_RELEASE_BIN" ]; then
        print_error "Backend release binary not found: $BACKEND_RELEASE_BIN"
        exit 1
    fi
    print_success "Backend release binary ready: $BACKEND_RELEASE_BIN"
}

build_parity_path() {
    if [ -n "$PARITY_PATH_OVERRIDE" ]; then
        echo "$PARITY_PATH_OVERRIDE"
        return
    fi

    local parity_path="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
    local npm_prefix=""
    if command -v npm >/dev/null 2>&1; then
        npm_prefix="$(npm config get prefix 2>/dev/null || true)"
        if [ -n "$npm_prefix" ] && [ "$npm_prefix" != "undefined" ] && [ "$npm_prefix" != "null" ]; then
            parity_path="$parity_path:$npm_prefix/bin"
        fi
    fi
    for runtime_bin in node npm npx; do
        if command -v "$runtime_bin" >/dev/null 2>&1; then
            local runtime_dir
            runtime_dir="$(dirname "$(command -v "$runtime_bin")")"
            parity_path="$parity_path:$runtime_dir"
        fi
    done
    parity_path="$parity_path:$HOME/.local/bin:$HOME/.npm-global/bin"
    echo "$parity_path"
}

run_provider_parity_smoke_check() {
    local parity_path="$1"
    local smoke_script="$PROJECT_ROOT/scripts/provider-smoke.sh"
    if [ ! -x "$smoke_script" ]; then
        print_warning "Provider smoke script not found or not executable: $smoke_script"
        return
    fi

    print_info "Running provider parity smoke check..."
    if ! "$smoke_script" --path "$parity_path"; then
        print_warning "Provider smoke check found missing commands. Auth may fail for some providers."
    fi
}

ensure_dev_admin_seed() {
    if [ "${ACPMS_DEV_SEED_ADMIN:-1}" != "1" ]; then
        return
    fi

    local admin_email="${ACPMS_DEV_ADMIN_EMAIL:-admin@acpms.local}"
    local admin_password="${ACPMS_DEV_ADMIN_PASSWORD:-acpms-dev-admin-123}"

    if [ "${#admin_password}" -lt 12 ]; then
        print_warning "Skipping dev admin seed: ACPMS_DEV_ADMIN_PASSWORD must be at least 12 chars"
        return
    fi

    print_info "Ensuring development admin exists: $admin_email"
    if ! (
        cd "$BACKEND_DIR"
        ADMIN_PASSWORD="$admin_password" cargo run --bin acpms-server -- --create-admin "$admin_email" >/dev/null 2>&1
    ); then
        print_warning "Failed to ensure development admin (continuing startup)"
        return
    fi
    print_success "Development admin ensured: $admin_email"
}

BACKEND_PID=""
FRONTEND_PID=""
STOP_REQUESTED=0

cleanup_children() {
    if [ -n "$BACKEND_PID" ] && kill -0 "$BACKEND_PID" >/dev/null 2>&1; then
        kill "$BACKEND_PID" >/dev/null 2>&1 || true
        wait "$BACKEND_PID" >/dev/null 2>&1 || true
    fi

    if [ -n "${FRONTEND_PID:-}" ] && kill -0 "$FRONTEND_PID" >/dev/null 2>&1; then
        kill "$FRONTEND_PID" >/dev/null 2>&1 || true
        wait "$FRONTEND_PID" >/dev/null 2>&1 || true
    fi
}

on_signal() {
    STOP_REQUESTED=1
    cleanup_children
    print_info "Stopped by user"
    exit 0
}

on_exit() {
    cleanup_children
}

start_dev_servers() {
    if [ "$PARITY_MODE" -eq 1 ]; then
        local parity_path
        parity_path="$(build_parity_path)"
        build_backend_release_binary
        run_provider_parity_smoke_check "$parity_path"

        print_info "Starting backend (production parity mode) at http://localhost:3000"
        (
            cd "$BACKEND_DIR"
            env -i \
                HOME="$HOME" \
                USER="${USER:-$(id -un)}" \
                SHELL="${SHELL:-/bin/sh}" \
                LANG="${LANG:-C.UTF-8}" \
                LC_ALL="${LC_ALL:-C.UTF-8}" \
                PATH="$parity_path" \
                APP_ENV=production \
                DATABASE_URL="$DATABASE_URL" \
                OPENCLAW_GATEWAY_ENABLED="${OPENCLAW_GATEWAY_ENABLED:-false}" \
                OPENCLAW_API_KEY="${OPENCLAW_API_KEY:-}" \
                OPENCLAW_WEBHOOK_URL="${OPENCLAW_WEBHOOK_URL:-}" \
                OPENCLAW_WEBHOOK_SECRET="${OPENCLAW_WEBHOOK_SECRET:-}" \
                OPENCLAW_ACTOR_USER_ID="${OPENCLAW_ACTOR_USER_ID:-}" \
                OPENCLAW_EVENT_RETENTION_HOURS="${OPENCLAW_EVENT_RETENTION_HOURS:-}" \
                ACPMS_FRONTEND_DIR="$FRONTEND_DIST" \
                ACPMS_SKILLS_DIR="$SKILLS_DIR" \
                "$BACKEND_RELEASE_BIN"
        ) &
        BACKEND_PID=$!
        FRONTEND_PID=""
        print_success "Backend started in parity mode (release binary + restricted env)"
        print_info "Parity PATH: $parity_path"
        print_info "Press Ctrl+C to stop"
    elif [ "$SINGLE_MODE" -eq 1 ]; then
        print_info "Starting backend (single binary mode) at http://localhost:3000"
        (
            cd "$BACKEND_DIR"
            export ACPMS_FRONTEND_DIR="$FRONTEND_DIST"
            export ACPMS_SKILLS_DIR="$SKILLS_DIR"
            cargo run --bin acpms-server
        ) &
        BACKEND_PID=$!
        FRONTEND_PID=""
        print_success "Backend started (serving frontend from $FRONTEND_DIST)"
        print_info "Press Ctrl+C to stop"
    else
        print_info "Starting backend at http://localhost:3000"
        (
            cd "$BACKEND_DIR"
            cargo run --bin acpms-server
        ) &
        BACKEND_PID=$!

        print_info "Starting frontend at http://localhost:5173"
        (
            cd "$FRONTEND_DIR"
            npm run dev
        ) &
        FRONTEND_PID=$!

        print_success "Development servers started"
        print_info "Press Ctrl+C to stop both"
    fi
}

monitor_processes() {
    while true; do
        if [ "$STOP_REQUESTED" -eq 1 ]; then
            return 0
        fi

        if ! kill -0 "$BACKEND_PID" >/dev/null 2>&1; then
            wait "$BACKEND_PID" || true
            if [ "$STOP_REQUESTED" -eq 1 ]; then
                return 0
            fi
            print_error "Backend process exited"
            return 1
        fi

        if [ -n "${FRONTEND_PID:-}" ] && ! kill -0 "$FRONTEND_PID" >/dev/null 2>&1; then
            wait "$FRONTEND_PID" || true
            if [ "$STOP_REQUESTED" -eq 1 ]; then
                return 0
            fi
            print_error "Frontend process exited"
            return 1
        fi

        sleep 1
    done
}

main() {
    require_command "docker" "https://www.docker.com/products/docker-desktop/"
    require_command "cargo" "https://rustup.rs/"
    require_command "node" "https://nodejs.org/"
    require_command "npm" "https://nodejs.org/"

    detect_compose_cmd
    ensure_infra_running
    wait_for_service_ready "postgres"
    wait_for_service_ready "minio"
    prepare_postgres_runtime

    print_info "PostgreSQL GUI port: localhost:$MAPPED_PG_PORT"

    if [ "$SINGLE_MODE" -eq 1 ]; then
        build_frontend_for_single_mode
    else
        ensure_frontend_deps
    fi

    ensure_dev_admin_seed

    trap on_exit EXIT
    trap on_signal INT TERM
    start_dev_servers
    monitor_processes
}

main "$@"
