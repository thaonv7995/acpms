#!/bin/bash
# Script to setup database and run tests

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${GREEN}=== Setting up test database and running tests ===${NC}\n"

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

# Detect PostgreSQL installation
PSQL_CMD=""
if command -v psql &> /dev/null; then
    PSQL_CMD="psql"
elif [ -f "/opt/homebrew/bin/psql" ]; then
    PSQL_CMD="/opt/homebrew/bin/psql"
elif [ -f "/usr/local/bin/psql" ]; then
    PSQL_CMD="/usr/local/bin/psql"
else
    echo -e "${RED}Error: psql not found. Please install PostgreSQL.${NC}"
    echo "On macOS: brew install postgresql@14"
    exit 1
fi

echo -e "${YELLOW}Using psql: $PSQL_CMD${NC}\n"

# Try to detect database connection
DB_URL=""
DB_USER=""
DB_PASSWORD=""

if [ -z "${DATABASE_URL:-}" ] && [ -f "$PROJECT_ROOT/.env" ]; then
    set -a
    . "$PROJECT_ROOT/.env"
    set +a
fi

if [ -n "${DATABASE_URL:-}" ]; then
    DB_URL="$(build_test_database_url "$DATABASE_URL")"
    DB_USER="$(printf '%s' "$DB_URL" | sed -n 's#.*://\([^:/]*\).*#\1#p')"
    DB_PASSWORD="$(printf '%s' "$DB_URL" | sed -n 's#.*://[^:]*:\([^@]*\)@.*#\1#p')"
fi

if [ -z "$DB_URL" ]; then
    DYNAMIC_PORT="$(resolve_docker_postgres_port || true)"
    if [ -n "$DYNAMIC_PORT" ]; then
        DB_URL="postgresql://acpms_user:acpms_password@127.0.0.1:${DYNAMIC_PORT}/acpms_test"
        DB_USER="acpms_user"
        DB_PASSWORD="acpms_password"
    fi
fi

# Try common connection strings
if [ -n "$DB_URL" ]; then
    echo -e "${YELLOW}Trying: $DB_URL${NC}"
    if PGPASSWORD="$DB_PASSWORD" $PSQL_CMD "$DB_URL" -c "SELECT 1;" &>/dev/null; then
        echo -e "${GREEN}✅ Connection successful!${NC}\n"
    else
        DB_URL=""
    fi
fi

if [ -z "$DB_URL" ]; then
    for user in "postgres" "$(whoami)"; do
        for pass in "postgres" ""; do
            if [ -z "$pass" ]; then
                test_url="postgresql://${user}@localhost:5432/postgres"
            else
                test_url="postgresql://${user}:${pass}@localhost:5432/postgres"
            fi

            echo -e "${YELLOW}Trying: $test_url${NC}"
            if PGPASSWORD="$pass" $PSQL_CMD "$test_url" -c "SELECT 1;" &>/dev/null; then
                DB_URL="$test_url"
                DB_USER="$user"
                DB_PASSWORD="$pass"
                echo -e "${GREEN}✅ Connection successful!${NC}\n"
                break 2
            fi
        done
    done
fi

if [ -z "$DB_URL" ]; then
    echo -e "${RED}Error: Could not connect to PostgreSQL.${NC}"
    echo "Please ensure PostgreSQL is running and accessible."
    echo "Try: brew services start postgresql@14"
    exit 1
fi

# Extract database name from URL or use default
DB_NAME="acpms_test"
TEST_DB_URL="${DB_URL%/*}/$DB_NAME"

# Create test database
echo -e "${YELLOW}Creating test database: $DB_NAME${NC}"
if PGPASSWORD="$DB_PASSWORD" $PSQL_CMD "$DB_URL" -c "CREATE DATABASE $DB_NAME;" 2>&1 | grep -q "already exists"; then
    echo -e "${GREEN}Database already exists${NC}\n"
else
    PGPASSWORD="$DB_PASSWORD" $PSQL_CMD "$DB_URL" -c "CREATE DATABASE $DB_NAME;" || {
        echo -e "${YELLOW}Database may already exist, continuing...${NC}\n"
    }
fi

# Set environment variables
export DATABASE_URL="$TEST_DB_URL"
export ENCRYPTION_KEY="AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
export JWT_SECRET="test-jwt-secret-key-for-testing-only"
export S3_ENDPOINT="http://localhost:9000"
export S3_PUBLIC_ENDPOINT="http://localhost:9000"
export S3_ACCESS_KEY="admin"
export S3_SECRET_KEY="adminpassword123"
export S3_REGION="us-east-1"
export S3_BUCKET_NAME="acpms-media"
export GITLAB_CLIENT_ID="test_client_id"
export GITLAB_CLIENT_SECRET="test_client_secret"
export GITLAB_REDIRECT_URI="http://localhost:3000/callback"

echo -e "${GREEN}Environment variables set${NC}"
echo -e "DATABASE_URL: $DATABASE_URL\n"

# Change to project root
cd "$(dirname "$0")/../.."

# Run tests
echo -e "${YELLOW}Running tests...${NC}\n"
cargo test --package acpms-server -- --ignored --nocapture --test-threads=1 "$@"

echo -e "\n${GREEN}Tests completed!${NC}"
