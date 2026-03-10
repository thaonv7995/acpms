#!/bin/bash
# Script to run API integration tests

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}=== ACPMS API Integration Tests ===${NC}\n"

PROJECT_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"

resolve_docker_postgres_port() {
    docker port acpms-postgres 5432 2>/dev/null | awk -F: '/127\\.0\\.0\\.1/ {print $NF; exit}'
}

build_test_database_url() {
    local base_url="$1"

    case "$base_url" in
        */acpms)
            printf '%s/acpms_test' "${base_url%/acpms}"
            ;;
        *)
            printf '%s' "$base_url"
            ;;
    esac
}

# Check if DATABASE_URL is set
if [ -z "$DATABASE_URL" ]; then
    if [ -f "$PROJECT_ROOT/.env" ]; then
        set -a
        . "$PROJECT_ROOT/.env"
        set +a
    fi

    if [ -n "${DATABASE_URL:-}" ]; then
        export DATABASE_URL="$(build_test_database_url "$DATABASE_URL")"
        echo -e "${YELLOW}DATABASE_URL loaded from .env and switched to test database${NC}"
    else
        DB_PORT="$(resolve_docker_postgres_port || true)"
        if [ -n "$DB_PORT" ]; then
            export DATABASE_URL="postgresql://acpms_user:acpms_password@127.0.0.1:${DB_PORT}/acpms_test"
            echo -e "${YELLOW}DATABASE_URL not set, using Docker PostgreSQL on localhost:${DB_PORT}${NC}"
        else
            echo -e "${RED}DATABASE_URL not set and Docker PostgreSQL port could not be determined${NC}"
            echo -e "${YELLOW}Export DATABASE_URL explicitly or start Docker Postgres first${NC}"
            exit 1
        fi
    fi
fi

echo -e "Database URL: ${DATABASE_URL}\n"

# Check if test database exists
DB_NAME=$(echo $DATABASE_URL | sed -n 's/.*\/\([^\/]*\)$/\1/p')
DB_HOST=$(echo $DATABASE_URL | sed -n 's/.*@\([^:]*\):.*/\1/p')
DB_PORT=$(echo $DATABASE_URL | sed -n 's/.*:\([0-9]*\)\/.*/\1/p')
DB_USER=$(echo $DATABASE_URL | sed -n 's/.*:\/\/\([^:]*\):.*/\1/p')

echo -e "${YELLOW}Checking test database connection...${NC}"
if ! PGPASSWORD=$(echo $DATABASE_URL | sed -n 's/.*:\/\/[^:]*:\([^@]*\)@.*/\1/p') psql -h "$DB_HOST" -p "${DB_PORT:-5432}" -U "$DB_USER" -lqt 2>/dev/null | cut -d \| -f 1 | grep -qw "$DB_NAME"; then
    echo -e "${RED}Test database '$DB_NAME' does not exist!${NC}"
    echo -e "${YELLOW}Creating test database...${NC}"
    PGPASSWORD=$(echo $DATABASE_URL | sed -n 's/.*:\/\/[^:]*:\([^@]*\)@.*/\1/p') createdb -h "$DB_HOST" -p "${DB_PORT:-5432}" -U "$DB_USER" "$DB_NAME" || {
        echo -e "${RED}Failed to create test database. Please create it manually:${NC}"
        echo -e "  createdb $DB_NAME"
        exit 1
    }
    echo -e "${GREEN}Test database created successfully${NC}\n"
else
    echo -e "${GREEN}Test database exists${NC}\n"
fi

# Change to project root
cd "$(dirname "$0")/../.."

echo -e "${YELLOW}Running tests...${NC}\n"

# Run tests with ignored flag (all tests are marked with #[ignore])
# Note: Integration tests in tests/ directory don't need --lib flag
cargo test --package acpms-server -- --ignored --test-threads=1 "$@"

echo -e "\n${GREEN}Tests completed!${NC}"
