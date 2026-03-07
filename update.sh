#!/usr/bin/env bash
# ACPMS Update - Upgrade existing installation to latest (or specified) release
#
# Usage:
#   bash update.sh                    # Update to latest release
#   bash update.sh --version v1.2.0    # Update to specific version
#   ACPMS_NONINTERACTIVE=1 bash update.sh  # Skip prompts (CI/CD)
#
# Prerequisites: curl, jq. Same as install.sh.
# Env: REPO, GITHUB_TOKEN (for private repo)
#
# Run from repo root or one-liner:
#   bash -c "$(curl -sSL https://raw.githubusercontent.com/thaonv7995/acpms/main/update.sh)"

set -e

if [ -z "${LC_ALL:-}" ] && [ -z "${LANG:-}" ]; then
  export LC_ALL=C.UTF-8 LANG=C.UTF-8 2>/dev/null || export LC_ALL=C LANG=C
fi

REPO="${ACPMS_REPO:-thaonv7995/acpms}"
log() { echo "[ACPMS] $*"; }
err() { echo "[ACPMS ERROR] $*" >&2; }
die() { err "$1"; exit 1; }

# Parse --version (e.g. --version v1.2.0 or --version=v1.2.0)
TARGET_VERSION=""
for arg in "$@"; do
  case "$arg" in
    --version=*)
      TARGET_VERSION="${arg#--version=}"
      ;;
    --version)
      TARGET_VERSION="__next__"
      ;;
    *)
      if [ "$TARGET_VERSION" = "__next__" ]; then
        TARGET_VERSION="$arg"
      fi
      ;;
  esac
done
[ "$TARGET_VERSION" = "__next__" ] && TARGET_VERSION=""

# Detect platform (same as install.sh)
detect_platform() {
  OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
  ARCH_RAW="$(uname -m)"
  case "$ARCH_RAW" in
    x86_64|amd64) ARCH_SUFFIX="amd64" ;;
    aarch64|arm64) ARCH_SUFFIX="arm64" ;;
    *) die "Unsupported architecture: $ARCH_RAW" ;;
  esac
  if [ "$OS" = "linux" ]; then
    OS_SUFFIX="linux"
    BASE_DIR="/opt/acpms"
    CONF_DIR="/etc/acpms"
    USE_SUDO="sudo"
  elif [ "$OS" = "darwin" ]; then
    OS_SUFFIX="macos"
    BASE_DIR="$HOME/.acpms"
    CONF_DIR="$HOME/.acpms/config"
    USE_SUDO=""
  else
    die "OS $OS is not supported."
  fi
  FRONTEND_DIR="$BASE_DIR/frontend"
  SKILLS_DIR="$BASE_DIR/.acpms/skills"
  BIN_PATH="$BASE_DIR/acpms-server"
  ENV_FILE="$CONF_DIR/.env"
  TARGET_BINARY="acpms-server-${OS_SUFFIX}-${ARCH_SUFFIX}"
}

# Detect existing installation
detect_install() {
  if [ -f "$ENV_FILE" ] && [ -f "$BIN_PATH" ]; then
    return 0
  fi
  return 1
}

# Stop service
stop_service() {
  log "Stopping ACPMS service..."
  if [ "$OS" = "linux" ] && command -v systemctl >/dev/null 2>&1; then
    $USE_SUDO systemctl stop acpms-server 2>/dev/null || true
  elif [ "$OS" = "darwin" ]; then
    launchctl unload "$HOME/Library/LaunchAgents/com.acpms.server.plist" 2>/dev/null || true
  fi
}

# Start service
start_service() {
  log "Starting ACPMS service..."
  if [ "$OS" = "linux" ] && command -v systemctl >/dev/null 2>&1; then
    $USE_SUDO systemctl start acpms-server
    log "Service started. Check: systemctl status acpms-server"
  elif [ "$OS" = "darwin" ]; then
    launchctl load "$HOME/Library/LaunchAgents/com.acpms.server.plist" 2>/dev/null || true
    log "Service started. Log: $BASE_DIR/acpms.log"
  else
    log "No systemd/launchd. Run manually: $BIN_PATH"
  fi
}

# Download one asset
download_asset() {
  local url="$1" out="$2" use_sudo="${3:-}" asset_id="${4:-}"
  local curl_opts=(-fSL)
  [ -t 2 ] && curl_opts+=(-#) || curl_opts+=(-s)
  if [ -n "${GITHUB_TOKEN:-}" ] && [ -n "$asset_id" ] && [ "$asset_id" != "null" ]; then
    url="https://api.github.com/repos/$REPO/releases/assets/$asset_id"
    curl_opts+=(-H "Accept: application/octet-stream" -H "Authorization: token $GITHUB_TOKEN")
  else
    [ -n "${GITHUB_TOKEN:-}" ] && curl_opts+=(-H "Authorization: token $GITHUB_TOKEN")
  fi
  if [ -n "$use_sudo" ]; then
    $USE_SUDO curl "${curl_opts[@]}" "$url" -o "$out"
  else
    curl "${curl_opts[@]}" "$url" -o "$out"
  fi
}

# Fetch release and download binary, frontend, skills
download_artifacts() {
  local api_url
  if [ -n "$TARGET_VERSION" ]; then
    api_url="https://api.github.com/repos/$REPO/releases/tags/$TARGET_VERSION"
    log "Fetching release $TARGET_VERSION..."
  else
    api_url="https://api.github.com/repos/$REPO/releases/latest"
    log "Fetching latest release..."
  fi

  local api_opts=(-sL)
  [ -n "${GITHUB_TOKEN:-}" ] && api_opts+=(-H "Authorization: token $GITHUB_TOKEN")
  local release_data
  release_data=$(curl "${api_opts[@]}" "$api_url") || true
  if [ -z "$release_data" ] || echo "$release_data" | jq -e '.message' >/dev/null 2>&1; then
    err "Failed to fetch release."
    [ -n "$TARGET_VERSION" ] && err "  Version $TARGET_VERSION may not exist."
    err "  Repo private? Set GITHUB_TOKEN. Releases: https://github.com/$REPO/releases"
    die "Cannot continue."
  fi

  local tag_name
  tag_name=$(echo "$release_data" | jq -r '.tag_name')
  log "Updating to $tag_name"

  local backend_url frontend_url skills_url backend_id frontend_id skills_id
  backend_url=$(echo "$release_data" | jq -r ".assets[] | select(.name==\"$TARGET_BINARY\") | .browser_download_url")
  frontend_url=$(echo "$release_data" | jq -r '.assets[] | select(.name=="acpms-frontend-dist.tar.gz") | .browser_download_url')
  skills_url=$(echo "$release_data" | jq -r '.assets[] | select(.name=="acpms-skills.tar.gz") | .browser_download_url')
  backend_id=$(echo "$release_data" | jq -r ".assets[] | select(.name==\"$TARGET_BINARY\") | .id")
  frontend_id=$(echo "$release_data" | jq -r '.assets[] | select(.name=="acpms-frontend-dist.tar.gz") | .id')
  skills_id=$(echo "$release_data" | jq -r '.assets[] | select(.name=="acpms-skills.tar.gz") | .id')

  [ -z "$backend_url" ] || [ "$backend_url" = "null" ] && die "Binary $TARGET_BINARY not found in $tag_name."
  [ -z "$frontend_url" ] || [ "$frontend_url" = "null" ] && die "Frontend not found in $tag_name."
  [ -z "$skills_url" ] || [ "$skills_url" = "null" ] && die "Skills not found in $tag_name."

  log "Downloading backend..."
  download_asset "$backend_url" "$BIN_PATH" "sudo" "$backend_id"
  $USE_SUDO chmod +x "$BIN_PATH"

  log "Downloading frontend..."
  download_asset "$frontend_url" /tmp/acpms-frontend.tar.gz "" "$frontend_id"
  $USE_SUDO tar -xzf /tmp/acpms-frontend.tar.gz -C "$FRONTEND_DIR"
  rm -f /tmp/acpms-frontend.tar.gz

  log "Downloading skills..."
  download_asset "$skills_url" /tmp/acpms-skills.tar.gz "" "$skills_id"
  $USE_SUDO mkdir -p "$SKILLS_DIR"
  $USE_SUDO tar -xzf /tmp/acpms-skills.tar.gz -C "$SKILLS_DIR"
  rm -f /tmp/acpms-skills.tar.gz
}

# Run migrations (uses existing .env)
run_migration() {
  log "Running database migrations..."
  if [ -n "$USE_SUDO" ]; then
    $USE_SUDO bash -c "set -a; [ -r '$ENV_FILE' ] && . '$ENV_FILE'; set +a; '$BIN_PATH' --migrate"
  else
    set -a
    # shellcheck disable=SC1090
    . "$ENV_FILE" 2>/dev/null || true
    set +a
    $BIN_PATH --migrate
  fi
}

# Main
main() {
  log "ACPMS Update - $REPO"
  command -v curl >/dev/null 2>&1 || die "curl is required."
  command -v jq >/dev/null 2>&1 || die "jq is required."

  detect_platform

  if ! detect_install; then
    die "ACPMS not found at $BASE_DIR. Run install.sh first."
  fi

  if [ -z "${ACPMS_NONINTERACTIVE:-}" ]; then
    read -rp "Update ACPMS to ${TARGET_VERSION:-latest}? [Y/n] " ans
    case "${ans:-y}" in
      [Nn]*) log "Cancelled."; exit 0 ;;
    esac
  fi

  stop_service
  download_artifacts
  run_migration
  start_service

  log "Update complete. ACPMS is running."
}

main
