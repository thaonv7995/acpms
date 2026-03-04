#!/bin/bash
# Script to run API integration tests

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}=== ACPMS API Integration Tests ===${NC}\n"

# Check if DATABASE_URL is set
if [ -z "$DATABASE_URL" ]; then
    echo -e "${YELLOW}DATABASE_URL not set, using default test database${NC}"
    export DATABASE_URL="postgresql://postgres:postgres@localhost:5432/acpms_test"
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
