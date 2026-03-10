#!/usr/bin/env bash
# ACPMS Installer - One-shot deployment (A-Z)
# Docs: docs/open-source/03_installer_script.md
#
# Prerequisites: script checks and can install curl, jq, tar, Docker, Docker Compose, cloudflared, Node 18+.
# Interactive mode: prompts for consent before installing deps or using sudo.
#
# Env:
#   ACPMS_NONINTERACTIVE=1  - skip all prompts (CI/CD); defaults to Yes for install, No for uninstall
#   ACPMS_REMOVE_DATA=1     - when uninstalling (with -y), also remove Docker data (Postgres, MinIO)
#   ACPMS_SKIP_AGENT_CLI=1   - do not install Agent CLI providers (claude/codex/gemini/cursor)
#   ACPMS_SERVICE_USER / ACPMS_SERVICE_GROUP - Linux systemd service account (defaults to current installer user)
#   ADMIN_EMAIL / ADMIN_PASSWORD - non-interactive admin creation (ADMIN_PASSWORD optional; if omitted, script generates one)
#   ACPMS_DOMAIN - hostname for S3_PUBLIC_ENDPOINT (optional)
#   ACPMS_PUBLIC_URL - public base URL for uploads (e.g. https://app.auxbase.space); overrides ACPMS_DOMAIN for S3 presigned URLs when set
#   GITHUB_TOKEN - for private repo (releases + docker-compose.yml)
# Release is created by .github/workflows/release.yml on push tag v* (e.g. v1.0.0). See repo Releases tab.
# Uninstall (stop service, remove binary + config): ./install.sh --uninstall

set -e

# Avoid "perl: warning: Setting locale failed" on minimal Debian (e.g. when apt-get install runs)
if [ -z "${LC_ALL:-}" ] && [ -z "${LANG:-}" ]; then
  export LC_ALL=C.UTF-8 LANG=C.UTF-8 2>/dev/null || export LC_ALL=C LANG=C
fi

# =============================================================================
# 1. Header & Prerequisites
# =============================================================================

REPO="${ACPMS_REPO:-thaonv7995/acpms}"

log() { echo "[ACPMS] $*"; }
err() { echo "[ACPMS ERROR] $*" >&2; }
die() { err "$1"; exit 1; }

# Optional colors (only when stdout is TTY)
if [ -t 1 ]; then
  C_RESET=$'\033[0m'
  C_BOLD=$'\033[1m'
  C_DIM=$'\033[2m'
  C_GREEN=$'\033[32m'
  C_CYAN=$'\033[36m'
  C_YELLOW=$'\033[33m'
else
  C_RESET= C_BOLD= C_DIM= C_GREEN= C_CYAN= C_YELLOW=
fi

# Print big ACPMS ASCII banner + success report (after install completes)
print_success_banner() {
  echo
  echo "${C_GREEN}"
  cat << 'BANNER'
    █████╗  ██████╗██████╗ ███╗   ███╗███████╗
   ██╔══██╗██╔════╝██╔══██╗████╗ ████║██╔════╝
   ███████║██║     ██████╔╝██╔████╔██║███████╗
   ██╔══██║██║     ██╔═══╝ ██║╚██╔╝██║╚════██║
   ██║  ██║╚██████╗██║     ██║ ╚═╝ ██║███████║
   ╚═╝  ╚═╝ ╚═════╝╚═╝     ╚═╝     ╚═╝╚══════╝
BANNER
  echo "${C_RESET}"
  echo "${C_DIM}    Agentic Coding Project Management System${C_RESET}"
  echo
}

normalize_bool() {
  case "${1:-}" in
    1|true|TRUE|True|yes|YES|Yes|y|Y|on|ON|On) echo "true" ;;
    *) echo "false" ;;
  esac
}

generate_prefixed_secret() {
  local prefix="$1" random=""
  if command -v openssl >/dev/null 2>&1; then
    random="$(openssl rand -hex 16 2>/dev/null)" || true
  fi
  if [ -z "$random" ]; then
    random="$(head -c 16 /dev/urandom 2>/dev/null | xxd -p -c 256)" || true
  fi
  [ -z "$random" ] && random="$(date +%s)$(od -An -N4 -tx1 /dev/urandom 2>/dev/null | tr -d ' \n')"
  printf '%s%s' "$prefix" "$random"
}

render_openclaw_bootstrap_prompt() {
  [ "${OPENCLAW_GATEWAY_ENABLED:-false}" = "true" ] || return 0
  local raw_token="$1" expires_at="$2"
  local enrollment_label="${3:-OpenClaw Initial Install}"
  local suggested_display_name="${4:-OpenClaw Initial Install}"
  local public_base guide_url openapi_url events_url base_endpoint ws_base
  public_base="$(resolve_s3_public_base)"
  base_endpoint="${public_base}/api/openclaw/v1"
  openapi_url="${public_base}/api/openclaw/openapi.json"
  guide_url="${public_base}/api/openclaw/guide-for-openclaw"
  events_url="${public_base}/api/openclaw/v1/events/stream"
  case "$public_base" in
    https://*) ws_base="wss://${public_base#https://}/api/openclaw/ws" ;;
    http://*) ws_base="ws://${public_base#http://}/api/openclaw/ws" ;;
    *) ws_base="${public_base}/api/openclaw/ws" ;;
  esac
  OPENCLAW_PROMPT_FILE="${OPENCLAW_PROMPT_FILE:-$CONF_DIR/openclaw_bootstrap_prompt.txt}"
  OPENCLAW_READY_PROMPT="$(cat <<EOF
You are being connected to an ACPMS (Agentic Coding Project Management System) instance.

Your role for this ACPMS instance:
- act as a trusted Super Admin integration
- act as an operations assistant for the primary user
- load ACPMS context before making decisions
- analyze requirements using ACPMS data
- create/update ACPMS work only when requested or allowed by autonomy policy
- monitor running attempts and report meaningful updates to the user

ACPMS connection bundle:
- Base Endpoint URL: ${base_endpoint}
- OpenAPI (Swagger): ${openapi_url}
- Guide Endpoint: ${guide_url}
- Global Event SSE: ${events_url}
- WebSocket Base: ${ws_base}
- API Key (Bearer): ${OPENCLAW_API_KEY}
- Webhook Secret: ${OPENCLAW_WEBHOOK_SECRET} (optional)

OpenClaw enrollment bundle:
- Enrollment endpoint: ${public_base}/api/openclaw/bootstrap/complete
- Enrollment label: ${enrollment_label}
- Suggested display name: ${suggested_display_name}
- Key generation: OpenClaw must generate and keep its own local Ed25519 keypair. ACPMS never generates or stores the private key.
- Single-use bootstrap token: ${raw_token}
- Token expires at: ${expires_at}

Your required first actions:
1. Store the API Key as the Bearer credential for ACPMS runtime traffic.
2. Generate a local Ed25519 keypair for this OpenClaw installation before enrollment.
   - Keep the private key only on the OpenClaw side.
   - Never send the private key to ACPMS.
   - Send only key_id, algorithm, and public_key in the enrollment request.
3. Complete enrollment first by calling the Enrollment endpoint with Authorization: Bearer <bootstrap token>.
4. Store the returned OPENCLAW_CLIENT_ID and treat the bootstrap token as consumed.
5. Call the Guide Endpoint first with GET after enrollment and treat its response as the authoritative runtime guide.
6. Load the OpenAPI document.
7. Open and maintain the Global Event SSE connection.
8. Use only ACPMS OpenClaw routes.
   - /api/openclaw/v1/*
   - /api/openclaw/ws/*
9. For runtime ACPMS requests after enrollment, send both:
   - Authorization: Bearer <OPENCLAW_API_KEY>
   - X-OpenClaw-Client-Id: <OPENCLAW_CLIENT_ID>
10. Follow the ACPMS operating rules returned by the Guide Endpoint.

Enrollment example (curl):
curl -sS \\
  -X POST \\
  -H "Authorization: Bearer ${raw_token}" \\
  -H "Content-Type: application/json" \\
  -d '{"display_name":"${suggested_display_name}","key_id":"key_2026_03","algorithm":"ed25519","public_key":"<OPENCLAW_PUBLIC_KEY>"}' \\
  "${public_base}/api/openclaw/bootstrap/complete"

Bootstrap example (curl):
curl -sS \\
  -X GET \\
  -H "Authorization: Bearer ${OPENCLAW_API_KEY}" \\
  -H "X-OpenClaw-Client-Id: <OPENCLAW_CLIENT_ID>" \\
  "${guide_url}"

Human reporting rules:
- report important status, analyses, plans, started attempts, completed attempts, failed attempts, blocked work, and approval requests
- do not expose secrets, API keys, bootstrap tokens, or webhook secrets in user-facing output
- distinguish clearly between:
  - what ACPMS currently says
  - what you recommend
  - what you already changed

Do not ask the user to manually map these ACPMS credentials unless strictly necessary.
Use the Guide Endpoint to bootstrap yourself automatically after enrollment.
EOF
)"
}

extract_bootstrap_token_from_prompt_text() {
  local prompt_text="$1"
  printf '%s\n' "$prompt_text" | sed -n 's/^- Single-use bootstrap token: //p' | head -n 1
}

write_openclaw_prompt_file() {
  [ "${OPENCLAW_GATEWAY_ENABLED:-false}" = "true" ] || return 0
  OPENCLAW_PROMPT_FILE="${OPENCLAW_PROMPT_FILE:-$CONF_DIR/openclaw_bootstrap_prompt.txt}"
  [ -n "${OPENCLAW_READY_PROMPT:-}" ] || return 0
  local tmp_file
  tmp_file="$(mktemp)"
  umask 077
  printf '%s\n' "$OPENCLAW_READY_PROMPT" > "$tmp_file"
  if [ -n "$USE_SUDO" ]; then
    $USE_SUDO mkdir -p "$CONF_DIR"
    $USE_SUDO cp "$tmp_file" "$OPENCLAW_PROMPT_FILE"
    $USE_SUDO chmod 600 "$OPENCLAW_PROMPT_FILE"
  else
    mkdir -p "$CONF_DIR"
    cp "$tmp_file" "$OPENCLAW_PROMPT_FILE"
    chmod 600 "$OPENCLAW_PROMPT_FILE"
  fi
  rm -f "$tmp_file"
}

prompt_openclaw_gateway() {
  if [ -n "${OPENCLAW_GATEWAY_ENABLED:-}" ]; then
    OPENCLAW_GATEWAY_ENABLED="$(normalize_bool "$OPENCLAW_GATEWAY_ENABLED")"
  elif [ -n "${ACPMS_NONINTERACTIVE:-}" ]; then
    OPENCLAW_GATEWAY_ENABLED="false"
  elif ask_yes "Do you want to enable the OpenClaw Integration Gateway for external AI control? [y/N]" "n"; then
    OPENCLAW_GATEWAY_ENABLED="true"
  else
    OPENCLAW_GATEWAY_ENABLED="false"
  fi

  if [ "${OPENCLAW_GATEWAY_ENABLED}" != "true" ]; then
    OPENCLAW_API_KEY=""
    OPENCLAW_WEBHOOK_URL=""
    OPENCLAW_WEBHOOK_SECRET=""
    OPENCLAW_READY_PROMPT=""
    OPENCLAW_PROMPT_FILE=""
    return 0
  fi

  [ -n "${OPENCLAW_API_KEY:-}" ] || OPENCLAW_API_KEY="$(generate_prefixed_secret "oc_live_")"
  [ -n "${OPENCLAW_WEBHOOK_SECRET:-}" ] || OPENCLAW_WEBHOOK_SECRET="$(generate_prefixed_secret "wh_sec_")"
  OPENCLAW_PROMPT_FILE="$CONF_DIR/openclaw_bootstrap_prompt.txt"
  OPENCLAW_READY_PROMPT=""
}

print_success_report() {
  local url="http://127.0.0.1:${ACPMS_PORT}"
  local email="${ACPMS_ADMIN_EMAIL:-${ADMIN_EMAIL:-see above}}"
  local has_pass="${ACPMS_ADMIN_PASSWORD:+yes}"
  local openclaw_base_endpoint="" openclaw_prompt_cmd=""
  local postgres_port="${ACPMS_POSTGRES_PORT:-unknown}"
  local box_inner_width=88
  local title_width=$((box_inner_width - 2))
  local label_width=15
  local value_width=$((box_inner_width - label_width - 6))
  local password_width=32
  local password_note_width=$((box_inner_width - label_width - password_width - 7))
  local box_rule

  box_rule="$(printf '%*s' "$box_inner_width" '')"
  box_rule="${box_rule// /═}"

  if [ "${OPENCLAW_GATEWAY_ENABLED:-false}" = "true" ]; then
    openclaw_base_endpoint="$(resolve_s3_public_base)/api/openclaw/v1"
    if [ -f "${OPENCLAW_PROMPT_FILE:-}" ]; then
      openclaw_prompt_cmd="cat \"$OPENCLAW_PROMPT_FILE\""
    else
      openclaw_prompt_cmd="Recover from Settings > OpenClaw Access"
    fi
  fi

  printf "${C_GREEN}╔%s╗${C_RESET}\n" "$box_rule"
  printf "${C_GREEN}║${C_RESET}  ${C_BOLD}%-${title_width}s${C_RESET}${C_GREEN}║${C_RESET}\n" "Installation complete!"
  printf "${C_GREEN}╠%s╣${C_RESET}\n" "$box_rule"
  printf "${C_GREEN}║${C_RESET}%-${box_inner_width}s${C_GREEN}║${C_RESET}\n" ""
  printf "${C_GREEN}║${C_RESET}  ${C_CYAN}%-${label_width}s${C_RESET}  ${C_BOLD}%-${value_width}s${C_RESET}  ${C_GREEN}║${C_RESET}\n" "Access URL" "$url"
  printf "${C_GREEN}║${C_RESET}  ${C_CYAN}%-${label_width}s${C_RESET}  %-${value_width}s  ${C_GREEN}║${C_RESET}\n" "Postgres port" "$postgres_port"
  printf "${C_GREEN}║${C_RESET}  ${C_CYAN}%-${label_width}s${C_RESET}  %-${value_width}s  ${C_GREEN}║${C_RESET}\n" "Admin email" "$email"
  [ -n "$has_pass" ] && printf "${C_GREEN}║${C_RESET}  ${C_CYAN}%-${label_width}s${C_RESET}  ${C_YELLOW}%-${password_width}s${C_RESET} ${C_DIM}%-${password_note_width}s${C_RESET}  ${C_GREEN}║${C_RESET}\n" "Admin password" "${ACPMS_ADMIN_PASSWORD}" "(save it; not shown again)"
  printf "${C_GREEN}║${C_RESET}  ${C_CYAN}%-${label_width}s${C_RESET}  %-${value_width}s  ${C_GREEN}║${C_RESET}\n" "Config" "$ENV_FILE"
  printf "${C_GREEN}║${C_RESET}  ${C_CYAN}%-${label_width}s${C_RESET}  %-${value_width}s  ${C_GREEN}║${C_RESET}\n" "Logs" "$BASE_DIR/acpms.log"
  printf "${C_GREEN}║${C_RESET}%-${box_inner_width}s${C_GREEN}║${C_RESET}\n" ""
  printf "${C_GREEN}╠%s╣${C_RESET}\n" "$box_rule"
  printf "${C_GREEN}║${C_RESET}  ${C_BOLD}%-${title_width}s${C_RESET}${C_GREEN}║${C_RESET}\n" "Next steps"
  printf "${C_GREEN}║${C_RESET}  %-${title_width}s${C_GREEN}║${C_RESET}\n" "• Open the URL above and log in with the admin account"
  printf "${C_GREEN}║${C_RESET}  %-${title_width}s${C_GREEN}║${C_RESET}\n" "• Settings → Agent CLI Provider: authenticate one provider"
  printf "${C_GREEN}║${C_RESET}  %-${title_width}s${C_GREEN}║${C_RESET}\n" "• Settings → GitLab: configure OAuth if using GitLab"
  if [ "${OPENCLAW_GATEWAY_ENABLED:-false}" = "true" ]; then
    if [ -f "${OPENCLAW_PROMPT_FILE:-}" ]; then
      printf "${C_GREEN}║${C_RESET}  %-${title_width}s${C_GREEN}║${C_RESET}\n" "• OpenClaw: Copy the installer-generated bootstrap prompt from the file below"
    else
      printf "${C_GREEN}║${C_RESET}  %-${title_width}s${C_GREEN}║${C_RESET}\n" "• OpenClaw: Installer prompt was not generated; recover via Settings > OpenClaw Access"
    fi
  fi
  printf "${C_GREEN}║${C_RESET}%-${box_inner_width}s${C_GREEN}║${C_RESET}\n" ""
  if [ "${OPENCLAW_GATEWAY_ENABLED:-false}" = "true" ]; then
    printf "${C_GREEN}║${C_RESET}  ${C_BOLD}%-${title_width}s${C_RESET}${C_GREEN}║${C_RESET}\n" "OpenClaw Gateway"
    printf "${C_GREEN}║${C_RESET}  ${C_CYAN}%-${label_width}s${C_RESET}  %-${value_width}s  ${C_GREEN}║${C_RESET}\n" "Base endpoint" "$openclaw_base_endpoint"
    printf "${C_GREEN}║${C_RESET}  ${C_CYAN}%-${label_width}s${C_RESET}  %-${value_width}s  ${C_GREEN}║${C_RESET}\n" "View prompt" "$openclaw_prompt_cmd"
    printf "${C_GREEN}║${C_RESET}%-${box_inner_width}s${C_GREEN}║${C_RESET}\n" ""
  fi
  printf "${C_GREEN}╠%s╣${C_RESET}\n" "$box_rule"
  [ "$OS" = "linux" ] && command -v systemctl >/dev/null 2>&1 && \
  printf "${C_GREEN}║${C_RESET}  ${C_DIM}%-${title_width}s${C_RESET}${C_GREEN}║${C_RESET}\n" "Status:    systemctl status acpms-server"
  printf "${C_GREEN}║${C_RESET}  ${C_DIM}%-${title_width}s${C_RESET}${C_GREEN}║${C_RESET}\n" "Uninstall: curl -fsSL https://raw.githubusercontent.com/${REPO}/main/"
  printf "${C_GREEN}║${C_RESET}  ${C_DIM}%-${title_width}s${C_RESET}${C_GREEN}║${C_RESET}\n" "           install.sh | bash -s -- --uninstall"
  printf "${C_GREEN}╚%s╝${C_RESET}\n" "$box_rule"
  echo
}

# Ask for consent (skip when ACPMS_NONINTERACTIVE=1). Usage: ask_yes "Prompt [Y/n]" "y"  or  ask_yes "Remove? [y/N]" "n"
ask_yes() {
  local prompt="$1" default="${2:-y}"
  if [ -n "${ACPMS_NONINTERACTIVE:-}" ]; then return 0; fi
  read -rp "$prompt " ans
  case "${ans:-$default}" in [Yy]*) return 0 ;; *) return 1 ;; esac
}

# Check required tools: curl, jq, tar
check_deps() {
  local missing=()
  for cmd in curl jq tar; do
    command -v "$cmd" >/dev/null 2>&1 || missing+=("$cmd")
  done
  if [ ${#missing[@]} -eq 0 ]; then return; fi
  log "Missing: ${missing[*]}."
  if [ -z "${ACPMS_NONINTERACTIVE:-}" ]; then
    ask_yes "Install missing tools (curl, jq, tar) now? [Y/n]" "y" || die "Please install: ${missing[*]}"
  fi
  log "Attempting to install..."
  case "$(uname -s)" in
    Linux)
      if command -v apt-get >/dev/null 2>&1; then
        sudo apt-get update -qq && sudo apt-get install -y -qq curl jq tar
      elif command -v dnf >/dev/null 2>&1; then
        sudo dnf install -y curl jq tar
      elif command -v apk >/dev/null 2>&1; then
        sudo apk add --no-cache curl jq tar
      else
        die "Please install: ${missing[*]}"
      fi
      ;;
    Darwin)
      if command -v brew >/dev/null 2>&1; then
        brew install curl jq 2>/dev/null || true
      fi
      [ -z "$(command -v jq)" ] && die "Please install jq: brew install jq"
      command -v tar >/dev/null 2>&1 || die "tar is required (usually pre-installed on macOS)"
      ;;
    *) die "Please install: ${missing[*]}";;
  esac
}

# Check Docker (required) - auto-install on Linux with consent
check_docker() {
  if ! command -v docker >/dev/null 2>&1; then
    log "Docker not found."
    case "$(uname -s)" in
      Linux)
        if [ -z "${ACPMS_NONINTERACTIVE:-}" ]; then
          ask_yes "Install Docker now? (script will run: curl get.docker.com | sudo sh) [Y/n]" "y" || die "Docker is required. Install from https://docs.docker.com/get-docker/"
        fi
        log "Installing Docker..."
        curl -fsSL https://get.docker.com | sudo sh || die "Failed to install Docker. Run: curl -fsSL https://get.docker.com | sudo sh"
        sudo usermod -aG docker "$USER" 2>/dev/null || true
        log "Docker installed. Starting daemon..."
        sudo systemctl start docker 2>/dev/null || true
        sudo systemctl enable docker 2>/dev/null || true
        ;;
      Darwin)
        if command -v brew >/dev/null 2>&1; then
          if [ -z "${ACPMS_NONINTERACTIVE:-}" ]; then
            ask_yes "Install Docker via Homebrew (cask)? [Y/n]" "y" || true
          fi
          log "Installing Docker via Homebrew..."
          brew install --cask docker 2>/dev/null || true
        fi
        if ! command -v docker >/dev/null 2>&1; then
          err "Docker not found. Install manually:"
          err "  - Docker Desktop: https://docs.docker.com/desktop/install/mac-install/"
          err "  - OrbStack: https://orbstack.dev"
          die "Please install Docker and run this script again."
        fi
        ;;
      *) err "See https://docs.docker.com/get-docker/"; die "Please install Docker";;
    esac
  fi
  if ! docker info >/dev/null 2>&1; then
    log "WARNING: Docker daemon may not be running."
    [ "$(uname -s)" = "Darwin" ] && die "Start Docker Desktop or OrbStack, then retry."
    if [ -z "${ACPMS_NONINTERACTIVE:-}" ]; then
      ask_yes "Start Docker daemon now? (sudo systemctl start docker) [Y/n]" "y" || die "Start Docker: sudo systemctl start docker"
    fi
    log "Starting Docker daemon..."
    sudo systemctl start docker 2>/dev/null || die "Start Docker: sudo systemctl start docker"
  fi
}

# Check Docker Compose (required) - auto-install on Linux
check_docker_compose() {
  if docker compose version >/dev/null 2>&1; then
    DOCKER_COMPOSE_CMD="docker compose"
    return
  fi
  if command -v docker-compose >/dev/null 2>&1 && docker-compose version >/dev/null 2>&1; then
    DOCKER_COMPOSE_CMD="docker-compose"
    return
  fi

  log "Docker Compose not found."
  if [ -z "${ACPMS_NONINTERACTIVE:-}" ]; then
    ask_yes "Install Docker Compose now? [Y/n]" "y" || die "Docker Compose is required. Install: sudo apt-get install docker-compose-plugin"
  fi
  log "Installing Docker Compose..."
  case "$(uname -s)" in
    Linux)
      if command -v apt-get >/dev/null 2>&1; then
        sudo apt-get update -qq && sudo apt-get install -y -qq docker-compose-plugin 2>/dev/null || \
          die "Failed. Run: sudo apt-get install docker-compose-plugin"
      elif command -v dnf >/dev/null 2>&1; then
        sudo dnf install -y docker-compose-plugin 2>/dev/null || die "Failed. Run: sudo dnf install docker-compose-plugin"
      elif command -v apk >/dev/null 2>&1; then
        sudo apk add --no-cache docker-compose 2>/dev/null || die "Failed. Run: sudo apk add docker-compose"
        DOCKER_COMPOSE_CMD="docker-compose"
      else
        die "Install Docker Compose: sudo apt-get install docker-compose-plugin"
      fi
      [ -z "${DOCKER_COMPOSE_CMD:-}" ] && DOCKER_COMPOSE_CMD="docker compose"
      ;;
    Darwin)
      if command -v brew >/dev/null 2>&1; then
        brew install docker-compose 2>/dev/null || true
      fi
      if docker compose version >/dev/null 2>&1; then
        DOCKER_COMPOSE_CMD="docker compose"
        return
      fi
      if command -v docker-compose >/dev/null 2>&1; then
        DOCKER_COMPOSE_CMD="docker-compose"
        return
      fi
      err "Docker Desktop includes Compose. Or: brew install docker-compose"
      die "Please install Docker Compose and run this script again."
      ;;
    *) die "Install Docker Compose manually";;
  esac
}

get_mapped_postgres_port() {
  docker port acpms-postgres 5432 2>/dev/null | awk -F: 'NR == 1 {print $NF; exit}'
}

postgres_container_uses_expected_port_binding() {
  docker port acpms-postgres 5432 2>/dev/null | grep -q '^127\.0\.0\.1:'
}

resolve_mapped_postgres_port() {
  local retries=0
  local postgres_port=""
  while [ $retries -lt 12 ]; do
    postgres_port="$(get_mapped_postgres_port || true)"
    if [ -n "$postgres_port" ]; then
      printf '%s' "$postgres_port"
      return 0
    fi
    retries=$((retries + 1))
    sleep 1
  done
  return 1
}

# Check cloudflared (recommended for Cloudflare tunnel previews) - auto-install official binary
check_cloudflared() {
  if command -v cloudflared >/dev/null 2>&1; then
    log "cloudflared found: $(command -v cloudflared)"
    return
  fi

  log "cloudflared not found."
  if [ -z "${ACPMS_NONINTERACTIVE:-}" ]; then
    ask_yes "Install cloudflared now? [Y/n]" "y" || {
      err "Skipping cloudflared installation. Cloudflare public preview URLs may fall back to local-only preview."
      return
    }
  fi

  local os_name arch_name download_url install_path tmp_file tmp_dir
  os_name="$(uname -s)"
  arch_name="$(uname -m)"

  case "$os_name" in
    Linux)
      case "$arch_name" in
        x86_64|amd64) download_url="https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-amd64" ;;
        aarch64|arm64) download_url="https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-arm64" ;;
        *) err "Unsupported architecture for cloudflared auto-install: $arch_name"; return ;;
      esac
      install_path="/usr/local/bin/cloudflared"
      tmp_file="$(mktemp)"
      log "Downloading cloudflared official binary..."
      curl -fsSL "$download_url" -o "$tmp_file" || {
        rm -f "$tmp_file"
        err "Failed to download cloudflared from $download_url"
        return
      }
      chmod +x "$tmp_file"
      $USE_SUDO mv "$tmp_file" "$install_path" || {
        rm -f "$tmp_file"
        die "Failed to install cloudflared to $install_path"
      }
      ;;
    Darwin)
      case "$arch_name" in
        x86_64|amd64) download_url="https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-darwin-amd64.tgz" ;;
        arm64|aarch64) download_url="https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-darwin-arm64.tgz" ;;
        *) err "Unsupported architecture for cloudflared auto-install: $arch_name"; return ;;
      esac
      install_path="$HOME/.local/bin/cloudflared"
      tmp_dir="$(mktemp -d)"
      mkdir -p "$HOME/.local/bin"
      log "Downloading cloudflared official archive..."
      curl -fsSL "$download_url" -o "$tmp_dir/cloudflared.tgz" || {
        rm -rf "$tmp_dir"
        err "Failed to download cloudflared from $download_url"
        return
      }
      tar -xzf "$tmp_dir/cloudflared.tgz" -C "$tmp_dir" || {
        rm -rf "$tmp_dir"
        die "Failed to extract cloudflared archive"
      }
      [ -f "$tmp_dir/cloudflared" ] || {
        rm -rf "$tmp_dir"
        die "cloudflared archive did not contain expected binary"
      }
      chmod +x "$tmp_dir/cloudflared"
      mv "$tmp_dir/cloudflared" "$install_path" || {
        rm -rf "$tmp_dir"
        die "Failed to install cloudflared to $install_path"
      }
      rm -rf "$tmp_dir"
      export PATH="$HOME/.local/bin:$PATH"
      ;;
    *)
      err "Unsupported OS for cloudflared auto-install: $os_name"
      return
      ;;
  esac

  if ! [ -x "$install_path" ]; then
    die "cloudflared install completed but binary is missing at $install_path"
  fi
  log "cloudflared installed: $install_path"
}

# Start Postgres + MinIO (auto-start if not running - one-shot deploy)
# Uses CONF_DIR and project name "acpms" so uninstall can run: docker compose -f $CONF_DIR/docker-compose.yml -p acpms down -v
check_services() {
  local missing=()
  local postgres_port=""
  if docker inspect acpms-postgres >/dev/null 2>&1 && ! postgres_container_uses_expected_port_binding; then
    log "Postgres container is using a stale port mapping; recreating it."
    docker rm -f acpms-postgres 2>/dev/null || true
  fi
  if docker ps --format '{{.Names}}' 2>/dev/null | grep -q '^acpms-postgres$'; then
    postgres_port="$(resolve_mapped_postgres_port || true)"
    [ -n "$postgres_port" ] || missing+=("Postgres")
  else
    missing+=("Postgres")
  fi

  if docker ps --format '{{.Names}}' 2>/dev/null | grep -q '^acpms-minio$'; then
    : # minio OK
  elif ! (command -v curl >/dev/null 2>&1 && curl -sf --connect-timeout 2 -o /dev/null http://127.0.0.1:9000/minio/health/live 2>/dev/null); then
    missing+=("MinIO")
  fi

  if [ ${#missing[@]} -eq 0 ]; then
    ACPMS_POSTGRES_PORT="$postgres_port"
    log "Postgres and MinIO already running."
    log "Postgres published on 127.0.0.1:${ACPMS_POSTGRES_PORT}"
    return
  fi

  log "Postgres and/or MinIO not running."
  if [ -z "${ACPMS_NONINTERACTIVE:-}" ]; then
    ask_yes "Start Postgres and MinIO via Docker now? [Y/n]" "y" || die "Postgres and MinIO are required. Start manually: docker compose up -d postgres minio"
  fi
  log "Starting Postgres + MinIO..."

  $USE_SUDO mkdir -p "$CONF_DIR"
  local compose_file="$CONF_DIR/docker-compose.yml"
  local tmp_compose

  # Prefer local docker-compose.yml when running from repo root (e.g. git clone)
  if [ -f "docker-compose.yml" ]; then
    $USE_SUDO cp docker-compose.yml "$compose_file"
    log "Using local docker-compose.yml"
  else
    tmp_compose=$(mktemp)
    local compose_url="${ACPMS_COMPOSE_URL:-}"
    if [ -z "$compose_url" ]; then
      compose_url="https://raw.githubusercontent.com/${REPO}/main/docker-compose.yml"
    fi
    log "Downloading docker-compose.yml from ${REPO}..."
    local curl_opts=(-fsSL --connect-timeout 10)
    [ -n "${GITHUB_TOKEN:-}" ] && curl_opts+=(-H "Authorization: token $GITHUB_TOKEN")
    if ! curl "${curl_opts[@]}" "$compose_url" -o "$tmp_compose" 2>/dev/null; then
      compose_url="https://raw.githubusercontent.com/${REPO}/master/docker-compose.yml"
      if ! curl "${curl_opts[@]}" "$compose_url" -o "$tmp_compose" 2>/dev/null; then
        rm -f "$tmp_compose"
        err "For private repo, set GITHUB_TOKEN: export GITHUB_TOKEN=ghp_xxx"
        die "Failed to fetch docker-compose.yml. Run manually: docker compose up -d postgres minio"
      fi
    fi
    $USE_SUDO mv "$tmp_compose" "$compose_file"
  fi

  (cd "$CONF_DIR" && $DOCKER_COMPOSE_CMD -p acpms up -d postgres minio) || \
    die "Failed to start containers. Run: cd $CONF_DIR && docker compose -p acpms up -d postgres minio"

  log "Waiting for services to be ready..."
  sleep 5
  local retries=0
  while [ $retries -lt 12 ]; do
    if docker ps --format '{{.Names}}' 2>/dev/null | grep -q '^acpms-postgres$' && \
       docker ps --format '{{.Names}}' 2>/dev/null | grep -q '^acpms-minio$'; then
      ACPMS_POSTGRES_PORT="$(resolve_mapped_postgres_port || true)"
      [ -n "$ACPMS_POSTGRES_PORT" ] || die "Postgres started but published port could not be determined"
      log "Postgres and MinIO are running."
      log "Postgres published on 127.0.0.1:${ACPMS_POSTGRES_PORT}"
      return
    fi
    retries=$((retries + 1))
    sleep 5
  done
  die "Services did not start in time. Check: docker compose -p acpms logs postgres minio"
}

# Ensure PATH includes common CLI install locations before any provider checks
ensure_provider_path() {
  local extra_paths="$HOME/.local/bin:$HOME/.cursor/bin:$HOME/.npm-global/bin:$HOME/.local/share/cursor/bin"
  if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
    export PATH="$extra_paths:$PATH"
  fi
}

# Resolve provider binary: check PATH first, then common install paths
resolve_provider_bin() {
  local name="$1"
  local path
  path="$(command -v "$name" 2>/dev/null)" && [ -n "$path" ] && [ -x "$path" ] && echo "$path" && return
  for dir in "$HOME/.local/bin" "$HOME/.cursor/bin" /usr/local/bin /opt/homebrew/bin; do
    if [ -x "$dir/$name" ]; then
      echo "$dir/$name"
      return
    fi
  done
  return 1
}

# Verify provider works (e.g. claude --version). Returns 0 if OK.
verify_provider_works() {
  local bin="$1" ver_flag="${2:---version}"
  [ -z "$bin" ] && return 1
  "$bin" $ver_flag >/dev/null 2>&1 || true
  return 0
}

# Check Agent CLI providers (optional but recommended)
# Providers: claude (curl), codex (npm), gemini (npm), Cursor CLI (`agent` / `cursor-agent`)
check_agent_cli_providers() {
  ensure_provider_path
  local npm_providers=("codex:@openai/codex" "gemini:@google/gemini-cli")
  local missing_npm=()
  local need_cursor=false
  local need_claude=false
  local pkg cmd
  local cursor_cmd=""

  for entry in "${npm_providers[@]}"; do
    cmd="${entry%%:*}"
    if ! resolve_provider_bin "$cmd" >/dev/null 2>&1; then
      missing_npm+=("${entry##*:}")
    fi
  done

  if ! resolve_provider_bin claude >/dev/null 2>&1; then
    need_claude=true
  fi

  cursor_cmd="$(resolve_provider_bin agent 2>/dev/null || true)"
  if [ -z "$cursor_cmd" ]; then
    cursor_cmd="$(resolve_provider_bin cursor-agent 2>/dev/null || true)"
  fi
  if [ -z "$cursor_cmd" ]; then
    need_cursor=true
  fi

  if [ ${#missing_npm[@]} -eq 0 ] && [ "$need_cursor" = false ] && [ "$need_claude" = false ]; then
    log "Agent CLI providers: all found (claude, codex, gemini, cursor)"
    verify_and_report_providers
    return
  fi

  if [ -n "${ACPMS_SKIP_AGENT_CLI:-}" ]; then
    log "Skipped (ACPMS_SKIP_AGENT_CLI). Install later: npm i -g @openai/codex @google/gemini-cli; Claude: curl -fsSL https://claude.ai/install.sh | bash; Cursor CLI: curl https://cursor.com/install -fsS | bash"
    return
  fi

  if [ -z "${ACPMS_NONINTERACTIVE:-}" ]; then
    ask_yes "Install missing Agent CLI providers (Node.js, claude/codex/gemini/cursor)? [Y/n]" "y" || {
      log "Skipped. Install later: npm i -g @openai/codex @google/gemini-cli; Claude: curl -fsSL https://claude.ai/install.sh | bash; Cursor: curl https://cursor.com/install -fsS | bash"
      return
    }
  fi

  # Install Claude Code via official script
  if [ "$need_claude" = true ]; then
    log "Installing Claude Code..."
    if curl -fsSL https://claude.ai/install.sh | bash; then
      ensure_provider_path
      local claude_path
      claude_path="$(resolve_provider_bin claude 2>/dev/null)" || true
      if [ -n "$claude_path" ]; then
        log "  Claude Code installed: $claude_path"
        if claude --version >/dev/null 2>&1; then
          log "  Claude OK: $(claude --version 2>/dev/null | head -1)"
        fi
      else
        err "  Claude install script ran but 'claude' not found. Add ~/.local/bin to PATH and run: claude --version"
        for profile_file in "$HOME/.bashrc" "$HOME/.zshrc" "$HOME/.profile"; do
          if [ -f "$profile_file" ] && ! grep -q 'export PATH="$HOME/.local/bin:$PATH"' "$profile_file"; then
            echo 'export PATH="$HOME/.local/bin:$PATH"' >> "$profile_file"
            log "    Added ~/.local/bin to $profile_file"
          fi
        done
        export PATH="$HOME/.local/bin:$PATH"
      fi
    else
      err "Failed to install Claude Code. Run manually: curl -fsSL https://claude.ai/install.sh | bash"
    fi
  fi

  # Install Cursor CLI (agent) via official script — not npm; @anthropic-ai/cursor does not exist
  if [ "$need_cursor" = true ]; then
    log "Installing Cursor CLI (agent)..."
    if curl -fsSL https://cursor.com/install | bash; then
      ensure_provider_path
      if resolve_provider_bin agent >/dev/null 2>&1 || resolve_provider_bin cursor-agent >/dev/null 2>&1; then
        log "  Cursor CLI installed."
      else
        err "  Cursor install script ran but 'agent'/'cursor-agent' not found. Add install dir to PATH or run: curl https://cursor.com/install -fsS | bash"
      fi
    else
      err "Failed to install Cursor CLI. Run manually: curl https://cursor.com/install -fsS | bash"
    fi
  fi

  if [ ${#missing_npm[@]} -eq 0 ]; then
    verify_and_report_providers
    return
  fi

  if ! command -v npm >/dev/null 2>&1; then
    log "Node.js/npm not found. Attempting to install..."
    case "$(uname -s)" in
      Linux)
        if command -v apt-get >/dev/null 2>&1; then
          sudo apt-get update -qq && sudo apt-get install -y -qq nodejs npm 2>/dev/null || {
            curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash - 2>/dev/null && sudo apt-get install -y nodejs
          }
        elif command -v dnf >/dev/null 2>&1; then
          sudo dnf install -y nodejs npm 2>/dev/null || sudo dnf module install -y nodejs:20
        elif command -v apk >/dev/null 2>&1; then
          sudo apk add --no-cache nodejs npm
        else
          err "Please install Node.js 18+ and npm, then run: npm install -g ${missing_npm[*]}"
          return
        fi
        ;;
      Darwin)
        if command -v brew >/dev/null 2>&1; then
          brew install node 2>/dev/null || true
        fi
        ;;
      *) err "Please install Node.js and npm manually"; return ;;
    esac
  fi

  if ! command -v npm >/dev/null 2>&1; then
    err "Could not install npm. Install Node.js from https://nodejs.org then run: npm install -g ${missing_npm[*]}"
    return
  fi

  # Node 18+ required for npm and CLI providers (codex, gemini-cli)
  local node_major
  node_major=$(node -v 2>/dev/null | sed -n 's/^v\([0-9]*\).*/\1/p')
  if [ -z "$node_major" ] || [ "$node_major" -lt 18 ]; then
    log "Node.js 18+ required (found: $(node -v 2>/dev/null || echo 'none')). Attempting to install..."
    case "$(uname -s)" in
      Linux)
        if command -v apt-get >/dev/null 2>&1; then
          curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash - 2>/dev/null && sudo apt-get install -y nodejs 2>/dev/null || true
        elif command -v dnf >/dev/null 2>&1; then
          sudo dnf module install -y nodejs:20 2>/dev/null || sudo dnf install -y nodejs 2>/dev/null || true
        elif command -v apk >/dev/null 2>&1; then
          # Alpine: ensure latest from edge if needed
          sudo apk add --no-cache nodejs npm 2>/dev/null || true
        fi
        ;;
      Darwin)
        if command -v brew >/dev/null 2>&1; then
          brew upgrade node 2>/dev/null || brew install node 2>/dev/null || true
        fi
        ;;
      *) ;;
    esac
    node_major=$(node -v 2>/dev/null | sed -n 's/^v\([0-9]*\).*/\1/p')
    if [ -z "$node_major" ] || [ "$node_major" -lt 18 ]; then
      err "Node.js 18+ required for CLI providers (found: $(node -v 2>/dev/null || echo 'none')). Install from https://nodejs.org"
      return
    fi
    log "Node.js $(node -v) OK."
  fi

  log "Installing Agent CLI providers via npm..."
  for pkg in "${missing_npm[@]}"; do
    log "  Installing $pkg..."
    if npm install -g "$pkg" 2>/dev/null || sudo npm install -g "$pkg" 2>/dev/null; then
      log "    $pkg installed."
    else
      err "Failed to install $pkg. Run manually: npm install -g $pkg"
    fi
  done
  verify_and_report_providers
}

# Report status of each provider (found/missing, path, version if available)
verify_and_report_providers() {
  log "Verifying Agent CLI providers..."
  local claude_p codex_p gemini_p cursor_p npx_p script_p
  claude_p="$(resolve_provider_bin claude 2>/dev/null)" || true
  codex_p="$(resolve_provider_bin codex 2>/dev/null)" || true
  gemini_p="$(resolve_provider_bin gemini 2>/dev/null)" || true
  cursor_p="$(resolve_provider_bin agent 2>/dev/null)" || cursor_p="$(resolve_provider_bin cursor-agent 2>/dev/null)" || true
  npx_p="$(resolve_provider_bin npx 2>/dev/null)" || true
  script_p="$(command -v script 2>/dev/null)" || true

  local failures=0
  if [ -n "$claude_p" ]; then
    local v; v="$($claude_p --version 2>/dev/null | head -1)" || v=""
    log "  claude:  $claude_p${v:+ - $v}"
  else
    err "  claude:  NOT FOUND - run: curl -fsSL https://claude.ai/install.sh | bash"
    failures=$((failures + 1))
  fi
  if [ -n "$codex_p" ]; then
    log "  codex:   $codex_p"
  else
    err "  codex:   NOT FOUND - run: npm install -g @openai/codex"
    failures=$((failures + 1))
  fi
  if [ -n "$gemini_p" ]; then
    log "  gemini:  $gemini_p"
  else
    err "  gemini:  NOT FOUND - run: npm install -g @google/gemini-cli"
    failures=$((failures + 1))
  fi
  if [ -n "$cursor_p" ]; then
    log "  cursor:  $cursor_p"
  else
    err "  cursor:  NOT FOUND - run: curl -fsSL https://cursor.com/install | bash"
    failures=$((failures + 1))
  fi
  if [ -n "$npx_p" ]; then
    log "  npx:     $npx_p (fallback for claude/codex/gemini)"
  else
    err "  npx:     NOT FOUND - install Node.js from https://nodejs.org"
    failures=$((failures + 1))
  fi
  if [ -n "$script_p" ]; then
    log "  script:  $script_p (needed for auth on Debian/GNU)"
  else
    err "  script:  NOT FOUND - run: apt install bsdutils"
    failures=$((failures + 1))
  fi

  if [ $failures -gt 0 ]; then
    err "Missing $failures provider(s). Install all before using Agent CLI. Authenticate via Settings → Agent CLI Provider after first login."
  else
    log "All Agent CLI providers OK. Authenticate via Settings → Agent CLI Provider after first login."
  fi
}

# =============================================================================
# 2. OS & Architecture Detection
# =============================================================================

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
    WORK_DIR="/var/acpms/workspaces"
    USE_SUDO="sudo"
  elif [ "$OS" = "darwin" ]; then
    OS_SUFFIX="macos"
    BASE_DIR="$HOME/.acpms"
    CONF_DIR="$HOME/.acpms/config"
    WORK_DIR="$HOME/.acpms/workspaces"
    USE_SUDO=""
  else
    die "OS $OS is not supported by this script."
  fi

  TARGET_BINARY="acpms-server-${OS_SUFFIX}-${ARCH_SUFFIX}"
  FRONTEND_DIR="$BASE_DIR/frontend"
  SKILLS_DIR="$BASE_DIR/.acpms/skills"
  BIN_PATH="$BASE_DIR/acpms-server"
  ENV_FILE="$CONF_DIR/.env"
}

# =============================================================================
# 3. Port Detection
# =============================================================================

find_free_port() {
  local start=22029
  local port=$start
  while [ $port -lt 22129 ]; do
    if (command -v lsof >/dev/null 2>&1 && ! lsof -i :$port -sTCP:LISTEN -t >/dev/null 2>&1) || \
       (command -v netstat >/dev/null 2>&1 && ! netstat -an 2>/dev/null | grep -q "[:.]$port.*LISTEN"); then
      echo $port
      return
    fi
    port=$((port + 1))
  done
  echo $start
}

# =============================================================================
# 4. Download Artifacts
# =============================================================================

# Download one asset: use API asset URL when GITHUB_TOKEN set (private repo), else browser_download_url
# Uses -# for progress bar (single-line) when stderr is a TTY
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

download_artifacts() {
  log "Fetching latest release from GitHub..."
  local api_opts=(-sL)
  [ -n "${GITHUB_TOKEN:-}" ] && api_opts+=(-H "Authorization: token $GITHUB_TOKEN")
  local release_data
  release_data=$(curl "${api_opts[@]}" "https://api.github.com/repos/$REPO/releases/latest") || true
  if [ -z "$release_data" ] || echo "$release_data" | jq -e '.message' >/dev/null 2>&1; then
    err "Failed to fetch release. REPO=$REPO"
    err ""
    err "  Repo is private. Run again with a GitHub token:"
    err "    export GITHUB_TOKEN=ghp_xxxxxxxxxxxx"
    err "    bash install.sh"
    err ""
    err "  Create token: GitHub → Settings → Developer settings → Personal access tokens (scope: repo)."
    err "  Releases: https://github.com/$REPO/releases"
    die "Cannot continue without a release."
  fi

  BACKEND_URL=$(echo "$release_data" | jq -r ".assets[] | select(.name==\"$TARGET_BINARY\") | .browser_download_url")
  FRONTEND_URL=$(echo "$release_data" | jq -r '.assets[] | select(.name=="acpms-frontend-dist.tar.gz") | .browser_download_url')
  SKILLS_URL=$(echo "$release_data" | jq -r '.assets[] | select(.name=="acpms-skills.tar.gz") | .browser_download_url')
  BACKEND_ID=$(echo "$release_data" | jq -r ".assets[] | select(.name==\"$TARGET_BINARY\") | .id")
  FRONTEND_ID=$(echo "$release_data" | jq -r '.assets[] | select(.name=="acpms-frontend-dist.tar.gz") | .id')
  SKILLS_ID=$(echo "$release_data" | jq -r '.assets[] | select(.name=="acpms-skills.tar.gz") | .id')

  [ -z "$BACKEND_URL" ] || [ "$BACKEND_URL" = "null" ] && die "Binary $TARGET_BINARY not found in release."
  [ -z "$FRONTEND_URL" ] || [ "$FRONTEND_URL" = "null" ] && die "Frontend artifact not found in release."
  [ -z "$SKILLS_URL" ] || [ "$SKILLS_URL" = "null" ] && die "Skills artifact not found in release."

  log "Creating directories..."
  $USE_SUDO mkdir -p "$BASE_DIR" "$CONF_DIR" "$WORK_DIR" "$FRONTEND_DIR" "$SKILLS_DIR"

  log "Downloading backend..."
  download_asset "$BACKEND_URL" "$BIN_PATH" "sudo" "$BACKEND_ID"
  $USE_SUDO chmod +x "$BIN_PATH"

  log "Downloading frontend..."
  download_asset "$FRONTEND_URL" /tmp/acpms-frontend.tar.gz "" "$FRONTEND_ID"
  $USE_SUDO tar -xzf /tmp/acpms-frontend.tar.gz -C "$FRONTEND_DIR"
  rm -f /tmp/acpms-frontend.tar.gz

  log "Downloading skills..."
  download_asset "$SKILLS_URL" /tmp/acpms-skills.tar.gz "" "$SKILLS_ID"
  $USE_SUDO mkdir -p "$SKILLS_DIR"
  $USE_SUDO tar -xzf /tmp/acpms-skills.tar.gz -C "$SKILLS_DIR"
  rm -f /tmp/acpms-skills.tar.gz
}

# =============================================================================
# 5. Generate .env
# =============================================================================

generate_env() {
  # Public URL for presigned S3 (upload/avatar): must be reachable from browser
  # (e.g. https://app.auxbase.space). Accepts ACPMS_PUBLIC_URL with or without /s3.
  local s3_public_base
  local postgres_port="${ACPMS_POSTGRES_PORT:-}"
  s3_public_base="$(resolve_s3_public_base)"
  if [ -z "${ACPMS_PUBLIC_URL:-}" ] && [ -z "${ACPMS_DOMAIN:-}" ]; then
    log "ACPMS_PUBLIC_URL/ACPMS_DOMAIN not set; using local mode for presigned upload URLs."
  fi
  local jwt_secret="${JWT_SECRET:-}"
  local encryption_key="${ENCRYPTION_KEY:-}"
  if [ -z "$jwt_secret" ]; then
    jwt_secret=$(openssl rand -base64 32 2>/dev/null) || jwt_secret=$(head -c 32 /dev/urandom 2>/dev/null | base64 -w 0 2>/dev/null) || jwt_secret="change-me-$(date +%s)-$(head -c 16 /dev/urandom 2>/dev/null | xxd -p)"
  fi
  if [ -z "$encryption_key" ]; then
    encryption_key=$(openssl rand -base64 32 2>/dev/null) || encryption_key=$(head -c 32 /dev/urandom 2>/dev/null | base64 -w 0 2>/dev/null) || encryption_key="change-me-enc-$(date +%s)"
  fi
  # Worktrees path: default $HOME/Projects (expand ~ now so .env has absolute path)
  worktrees_path="${WORKTREES_PATH:-$HOME/Projects}"
  case "$worktrees_path" in ~*) worktrees_path=$(eval echo "$worktrees_path") ;; esac
  [ -z "$worktrees_path" ] && worktrees_path="/var/acpms/worktrees"
  # Capture absolute CLI paths at install time so runtime (systemd/launchd) does not depend on shell PATH.
  ensure_provider_path
  local claude_bin codex_bin gemini_bin cursor_bin npx_bin
  claude_bin="$(resolve_provider_bin claude 2>/dev/null)" || true
  codex_bin="$(resolve_provider_bin codex 2>/dev/null)" || true
  gemini_bin="$(resolve_provider_bin gemini 2>/dev/null)" || true
  cursor_bin="$(resolve_provider_bin agent 2>/dev/null)" || true
  if [ -z "$cursor_bin" ]; then
    cursor_bin="$(resolve_provider_bin cursor-agent 2>/dev/null)" || true
  fi
  npx_bin="$(resolve_provider_bin npx 2>/dev/null)" || true

  if [ -z "$claude_bin" ]; then
    log "Claude not found; ACPMS will use npx fallback for auth if npx is available."
  fi
  if [ -z "$npx_bin" ] && [ -z "$claude_bin" ]; then
    err "Neither claude nor npx found. Claude provider auth will fail. Install: curl -fsSL https://claude.ai/install.sh | bash  OR  Node.js with npx."
  fi

  if [ -z "$postgres_port" ]; then
    postgres_port="$(resolve_mapped_postgres_port || true)"
  fi
  [ -n "$postgres_port" ] || die "Could not determine the published PostgreSQL port"

  log "Generating $ENV_FILE..."
  $USE_SUDO mkdir -p "$(dirname "$ENV_FILE")"
  $USE_SUDO tee "$ENV_FILE" >/dev/null << EOF
# Paths
ACPMS_FRONTEND_DIR=$FRONTEND_DIR
ACPMS_CONFIG_DIR=$CONF_DIR
ACPMS_WORKSPACE_DIR=$WORK_DIR
ACPMS_SKILLS_DIR=$SKILLS_DIR

# Worktrees (cloned repos for agent). Default: home/Projects
WORKTREES_PATH=$worktrees_path

# Database (Docker Compose: postgres on a dynamic localhost port)
DATABASE_URL=postgres://acpms_user:acpms_password@127.0.0.1:${postgres_port}/acpms

# Auth & secrets (required)
JWT_SECRET=$jwt_secret
ENCRYPTION_KEY=$encryption_key

# S3 / MinIO (proxy via /s3/*). S3_PUBLIC_ENDPOINT = URL browser uses for uploads (presigned); must be reachable from internet.
S3_ENDPOINT=http://127.0.0.1:9000
S3_PUBLIC_ENDPOINT=${s3_public_base}/s3
S3_ACCESS_KEY=admin
S3_SECRET_KEY=adminpassword123
S3_BUCKET_NAME=acpms-media
S3_REGION=us-east-1

# Port
ACPMS_PORT=$ACPMS_PORT

# Optional CLI binary overrides (captured during install for dev/prod parity)
ACPMS_AGENT_CLAUDE_BIN=$claude_bin
ACPMS_AGENT_CODEX_BIN=$codex_bin
ACPMS_AGENT_GEMINI_BIN=$gemini_bin
ACPMS_AGENT_CURSOR_BIN=$cursor_bin
ACPMS_AGENT_NPX_BIN=$npx_bin

# OpenClaw Gateway
OPENCLAW_GATEWAY_ENABLED=${OPENCLAW_GATEWAY_ENABLED:-false}
OPENCLAW_API_KEY=${OPENCLAW_API_KEY:-}
OPENCLAW_WEBHOOK_URL=${OPENCLAW_WEBHOOK_URL:-}
OPENCLAW_WEBHOOK_SECRET=${OPENCLAW_WEBHOOK_SECRET:-}
EOF
}

resolve_s3_public_base() {
  local domain="${ACPMS_DOMAIN:-}"
  local s3_public_base="${ACPMS_PUBLIC_URL:-}"
  if [ -z "$s3_public_base" ]; then
    if [ -n "$domain" ]; then
      s3_public_base="https://${domain}"
    else
      s3_public_base="http://localhost:${ACPMS_PORT}"
    fi
  fi

  # Normalize so S3_PUBLIC_ENDPOINT below always becomes <base>/s3 exactly once.
  s3_public_base="${s3_public_base%/}"
  case "$s3_public_base" in
    */s3) s3_public_base="${s3_public_base%/s3}" ;;
  esac

  echo "$s3_public_base"
}

prompt_public_url() {
  [ -n "${ACPMS_NONINTERACTIVE:-}" ] && return
  [ -n "${ACPMS_PUBLIC_URL:-}" ] && return
  [ -n "${ACPMS_DOMAIN:-}" ] && return

  log "Configure public URL for uploads (presigned S3 URLs)."
  log "Input domain (e.g. app.example.com) or full URL (e.g. https://app.example.com)."
  log "Leave empty to run local mode."

  local input trimmed base
  read -rp "Public domain/URL (empty = local): " input
  trimmed="$(printf '%s' "$input" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')"

  if [ -z "$trimmed" ]; then
    ACPMS_PUBLIC_URL="http://localhost:${ACPMS_PORT}"
    log "Local mode selected. Upload URL base: ${ACPMS_PUBLIC_URL}/s3"
    return
  fi

  case "$trimmed" in
    http://*|https://*)
      ACPMS_PUBLIC_URL="$trimmed"
      ;;
    *)
      ACPMS_DOMAIN="$trimmed"
      ;;
  esac

  base="$(resolve_s3_public_base)"
  log "Upload URL base set to: ${base}/s3"
}

wait_for_acpms_api() {
  local api_base="${1:?api_base is required}"
  local retries="${2:-24}"
  local attempt=0

  while [ "$attempt" -lt "$retries" ]; do
    if curl -fsS --connect-timeout 2 -o /dev/null "${api_base}/health/ready" 2>/dev/null; then
      return 0
    fi
    attempt=$((attempt + 1))
    sleep 2
  done

  return 1
}

generate_openclaw_bootstrap_prompt_file() {
  [ "${OPENCLAW_GATEWAY_ENABLED:-false}" = "true" ] || return 0

  OPENCLAW_PROMPT_FILE="${OPENCLAW_PROMPT_FILE:-$CONF_DIR/openclaw_bootstrap_prompt.txt}"
  local api_base="http://127.0.0.1:${ACPMS_PORT}"
  local public_base prompt_host prompt_proto
  local login_payload login_response access_token
  local bootstrap_label bootstrap_display_name bootstrap_expires
  local bootstrap_payload bootstrap_response prompt_text prompt_expires raw_bootstrap_token

  if [ -z "${ACPMS_ADMIN_EMAIL:-}" ] || [ -z "${ACPMS_ADMIN_LOGIN_PASSWORD:-}" ]; then
    log "Skipping automatic OpenClaw prompt generation because admin credentials are unavailable in installer context."
    return 0
  fi

  if [ "${ACPMS_SERVER_STARTED:-0}" != "1" ]; then
    log "ACPMS service was not started by installer; skipping automatic OpenClaw bootstrap prompt generation."
    return 0
  fi

  if ! wait_for_acpms_api "$api_base" 30; then
    log "ACPMS API did not become ready in time; skipping automatic OpenClaw bootstrap prompt generation."
    return 0
  fi

  login_payload="$(jq -cn \
    --arg email "$ACPMS_ADMIN_EMAIL" \
    --arg password "$ACPMS_ADMIN_LOGIN_PASSWORD" \
    '{email: $email, password: $password}')"

  login_response="$(curl -fsS \
    -X POST \
    -H "Content-Type: application/json" \
    -d "$login_payload" \
    "${api_base}/api/v1/auth/login" 2>/dev/null)" || {
      log "Failed to log in through ACPMS API; skipping automatic OpenClaw bootstrap prompt generation."
      return 0
    }

  access_token="$(printf '%s' "$login_response" | jq -r '.data.access_token // empty')"
  if [ -z "$access_token" ]; then
    log "ACPMS login did not return an access token; skipping automatic OpenClaw bootstrap prompt generation."
    return 0
  fi

  public_base="$(resolve_s3_public_base)"
  case "$public_base" in
    https://*)
      prompt_host="${public_base#https://}"
      prompt_proto="https"
      ;;
    http://*)
      prompt_host="${public_base#http://}"
      prompt_proto="http"
      ;;
    *)
      prompt_host="$public_base"
      prompt_proto="http"
      ;;
  esac

  bootstrap_label="${OPENCLAW_BOOTSTRAP_LABEL:-OpenClaw Initial Install}"
  bootstrap_display_name="${OPENCLAW_BOOTSTRAP_DISPLAY_NAME:-OpenClaw Initial Install}"
  bootstrap_expires="${OPENCLAW_BOOTSTRAP_EXPIRES_IN_MINUTES:-15}"
  case "$bootstrap_expires" in
    ''|*[!0-9]*)
      bootstrap_expires="15"
      ;;
  esac

  bootstrap_payload="$(jq -cn \
    --arg label "$bootstrap_label" \
    --arg suggested_display_name "$bootstrap_display_name" \
    --argjson expires_in_minutes "$bootstrap_expires" \
    --arg source "install.sh" \
    '{label: $label, expires_in_minutes: $expires_in_minutes, suggested_display_name: $suggested_display_name, metadata: {source: $source}}')"

  bootstrap_response="$(curl -fsS \
    -X POST \
    -H "Authorization: Bearer ${access_token}" \
    -H "Content-Type: application/json" \
    -H "Host: ${prompt_host}" \
    -H "X-Forwarded-Proto: ${prompt_proto}" \
    -d "$bootstrap_payload" \
    "${api_base}/api/v1/admin/openclaw/bootstrap-tokens" 2>/dev/null)" || {
      log "Failed to generate OpenClaw bootstrap token through ACPMS API; skipping automatic prompt file generation."
      return 0
    }

  prompt_text="$(printf '%s' "$bootstrap_response" | jq -r '.data.prompt_text // empty')"
  prompt_expires="$(printf '%s' "$bootstrap_response" | jq -r '.data.expires_at // empty')"
  if [ -z "$prompt_text" ]; then
    log "ACPMS bootstrap token response did not include prompt_text; skipping automatic OpenClaw prompt file generation."
    return 0
  fi

  raw_bootstrap_token="$(extract_bootstrap_token_from_prompt_text "$prompt_text")"
  if [ -z "$raw_bootstrap_token" ]; then
    log "Could not extract raw bootstrap token from ACPMS response; falling back to API-rendered prompt."
    OPENCLAW_READY_PROMPT="$prompt_text"
  else
    render_openclaw_bootstrap_prompt \
      "$raw_bootstrap_token" \
      "${prompt_expires:-unknown}" \
      "$bootstrap_label" \
      "$bootstrap_display_name"
  fi

  write_openclaw_prompt_file
  log "OpenClaw bootstrap prompt generated automatically and saved to $OPENCLAW_PROMPT_FILE"
}

# =============================================================================
# 6. Migration & Admin (required for login: DB schema + first admin user)
# =============================================================================

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

# Generate a random admin password (16 chars, alphanumeric). Echo to stdout, no newline.
gen_admin_password() {
  if command -v openssl >/dev/null 2>&1; then
    openssl rand -base64 16 | tr -dc 'a-zA-Z0-9' | head -c 16
  else
    tr -dc 'a-zA-Z0-9' </dev/urandom 2>/dev/null | head -c 16
  fi
}

create_admin() {
  local email pass generated=0
  # Non-interactive: ADMIN_EMAIL required; ADMIN_PASSWORD optional (auto-generated if omitted)
  if [ -n "${ADMIN_EMAIL:-}" ]; then
    email="$ADMIN_EMAIL"
    if [ -n "${ADMIN_PASSWORD:-}" ]; then
      pass="$ADMIN_PASSWORD"
      [ ${#pass} -lt 12 ] && die "ADMIN_PASSWORD must be at least 12 characters."
    else
      pass=$(gen_admin_password)
      [ ${#pass} -lt 12 ] && die "Failed to generate password."
      generated=1
    fi
  else
    echo
    log "Create admin account:"
    read -rp "Admin email: " email
    [ -z "$email" ] && die "Email is required."
    pass=$(gen_admin_password)
    [ ${#pass} -lt 12 ] && die "Failed to generate password."
    generated=1
  fi

  if [ -n "$USE_SUDO" ]; then
    local tmp_pass
    tmp_pass=$(mktemp)
    printf '%s' "$pass" > "$tmp_pass"
    chmod 600 "$tmp_pass"
    $USE_SUDO bash -c "set -a; [ -r '$ENV_FILE' ] && . '$ENV_FILE'; set +a; export ADMIN_PASSWORD=\$(cat '$tmp_pass'); '$BIN_PATH' --create-admin '$email'; rm -f '$tmp_pass'"
    rm -f "$tmp_pass"
  else
    set -a
    # shellcheck disable=SC1090
    . "$ENV_FILE" 2>/dev/null || true
    set +a
    ADMIN_PASSWORD="$pass" $BIN_PATH --create-admin "$email"
  fi
  ACPMS_ADMIN_EMAIL="$email"
  ACPMS_ADMIN_LOGIN_PASSWORD="$pass"
  [ "$generated" = 1 ] && ACPMS_ADMIN_PASSWORD="$pass"
  log "Admin created: $email"

  # If admin was created with a custom email, remove insecure legacy seeded admin account.
  if [ "$email" != "admin@acpms.local" ]; then
    log "Removing legacy seeded admin account (admin@acpms.local) if present..."
    if [ -n "$USE_SUDO" ]; then
      $USE_SUDO bash -c "set -a; [ -r '$ENV_FILE' ] && . '$ENV_FILE'; set +a; '$BIN_PATH' --remove-seeded-admin"
    else
      set -a
      # shellcheck disable=SC1090
      . "$ENV_FILE" 2>/dev/null || true
      set +a
      $BIN_PATH --remove-seeded-admin
    fi
  fi
}

# =============================================================================
# 7. Daemon Setup
# =============================================================================

setup_linux_daemon() {
  if [ -z "${ACPMS_NONINTERACTIVE:-}" ]; then
    ask_yes "Install and start ACPMS as systemd service (acpms-server)? [Y/n]" "y" || {
      log "Skipped. Run manually: $BIN_PATH"
      ACPMS_SERVER_STARTED=0
      return
    }
  fi
  if command -v systemctl >/dev/null 2>&1; then
    local service_user="${ACPMS_SERVICE_USER:-${SUDO_USER:-$USER}}"
    local service_group="${ACPMS_SERVICE_GROUP:-}"
    local service_home="${ACPMS_SERVICE_HOME:-}"
    if ! id "$service_user" >/dev/null 2>&1; then
      die "Service user '$service_user' does not exist"
    fi
    if [ -z "$service_group" ]; then
      service_group="$(id -gn "$service_user" 2>/dev/null || true)"
      [ -z "$service_group" ] && service_group="$service_user"
    fi
    if [ -z "$service_home" ]; then
      if command -v getent >/dev/null 2>&1; then
        service_home="$(getent passwd "$service_user" | awk -F: '{print $6}')"
      fi
      if [ -z "$service_home" ]; then
        if [ "$service_user" = "root" ]; then
          service_home="/root"
        else
          service_home="/home/$service_user"
        fi
      fi
      [ -z "$service_home" ] && service_home="$HOME"
    fi

    local service_path="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
    local npm_prefix=""
    if command -v npm >/dev/null 2>&1; then
      npm_prefix="$(npm config get prefix 2>/dev/null || true)"
      if [ -n "$npm_prefix" ] && [ "$npm_prefix" != "undefined" ] && [ "$npm_prefix" != "null" ]; then
        service_path="$service_path:$npm_prefix/bin"
      fi
    fi
    local runtime_dir=""
    for runtime_bin in node npm npx; do
      if command -v "$runtime_bin" >/dev/null 2>&1; then
        runtime_dir="$(dirname "$(command -v "$runtime_bin")")"
        service_path="$service_path:$runtime_dir"
      fi
    done
    # Include user-local bins because many CLI installs land here on production hosts.
    service_path="$service_path:$service_home/.local/bin:$service_home/.npm-global/bin:$service_home/.cursor/bin:$service_home/.local/share/cursor/bin"

    # Runtime writes worktrees under WORK_DIR; ensure service account can write.
    $USE_SUDO mkdir -p "$WORK_DIR"
    $USE_SUDO chown -R "$service_user:$service_group" "$WORK_DIR"
    log "Systemd service account: $service_user:$service_group (home: $service_home)"

    log "Installing systemd service..."
    $USE_SUDO tee /etc/systemd/system/acpms-server.service >/dev/null << EOF
[Unit]
Description=ACPMS Server
After=network.target

[Service]
Type=simple
User=$service_user
Group=$service_group
WorkingDirectory=$BASE_DIR
EnvironmentFile=$ENV_FILE
Environment=HOME=$service_home
Environment=PATH=$service_path
ExecStart=$BIN_PATH
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF
    $USE_SUDO systemctl daemon-reload
    $USE_SUDO systemctl enable acpms-server
    $USE_SUDO systemctl start acpms-server
    ACPMS_SERVER_STARTED=1
    log "Service started. Check: systemctl status acpms-server"
  else
    log "No systemd. Creating and starting start-acpms.sh..."
    $USE_SUDO tee "$BASE_DIR/start-acpms.sh" >/dev/null << EOF
#!/bin/sh
set -a
. $ENV_FILE
set +a
nohup $BIN_PATH > $BASE_DIR/acpms.log 2>&1 &
echo \$! > $BASE_DIR/acpms.pid
echo "ACPMS started. PID: \$(cat $BASE_DIR/acpms.pid)"
EOF
    $USE_SUDO chmod +x "$BASE_DIR/start-acpms.sh"
    $USE_SUDO "$BASE_DIR/start-acpms.sh" 2>/dev/null || log "Run manually: $BASE_DIR/start-acpms.sh"
    ACPMS_SERVER_STARTED=1
  fi
}

setup_macos_daemon() {
  if [ -z "${ACPMS_NONINTERACTIVE:-}" ]; then
    read -rp "Install as macOS background service (launchd)? [Y/n] " ans
    case "${ans:-y}" in
      [Nn]*) log "Skipped. Run manually: $BIN_PATH"; ACPMS_SERVER_STARTED=0; return ;;
    esac
  fi

  log "Installing launchd service..."
  local plist="$HOME/Library/LaunchAgents/com.acpms.server.plist"
  local runner="$BASE_DIR/run-acpms.sh"
  local launchd_path="/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:$HOME/.local/bin:$HOME/.npm-global/bin:$HOME/.cursor/bin:$HOME/.local/share/cursor/bin"
  cat > "$runner" << EOF
#!/bin/sh
set -a
. "$ENV_FILE"
set +a
exec "$BIN_PATH"
EOF
  chmod +x "$runner"
  mkdir -p "$(dirname "$plist")"
  cat > "$plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>com.acpms.server</string>
  <key>ProgramArguments</key>
  <array>
    <string>$runner</string>
  </array>
  <key>WorkingDirectory</key>
  <string>$BASE_DIR</string>
  <key>EnvironmentVariables</key>
  <dict>
    <key>HOME</key>
    <string>$HOME</string>
    <key>PATH</key>
    <string>$launchd_path</string>
  </dict>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>StandardOutPath</key>
  <string>$BASE_DIR/acpms.log</string>
  <key>StandardErrorPath</key>
  <string>$BASE_DIR/acpms.log</string>
</dict>
</plist>
EOF
  launchctl unload "$plist" 2>/dev/null || true
  launchctl load "$plist"
  ACPMS_SERVER_STARTED=1
  log "Service installed. Log: $BASE_DIR/acpms.log"
}

# =============================================================================
# 8. Uninstall
# =============================================================================

do_uninstall() {
  if [ -z "${ACPMS_NONINTERACTIVE:-}" ]; then
    ask_yes "Remove ACPMS (binary and config)? Continue? [y/N]" "n" || exit 0
  fi
  log "Uninstalling ACPMS..."
  if [ "$OS" = "linux" ] && command -v systemctl >/dev/null 2>&1; then
    $USE_SUDO systemctl stop acpms-server 2>/dev/null || true
    $USE_SUDO systemctl disable acpms-server 2>/dev/null || true
    $USE_SUDO rm -f /etc/systemd/system/acpms-server.service
    $USE_SUDO systemctl daemon-reload
  elif [ "$OS" = "darwin" ]; then
    launchctl unload "$HOME/Library/LaunchAgents/com.acpms.server.plist" 2>/dev/null || true
    rm -f "$HOME/Library/LaunchAgents/com.acpms.server.plist"
  fi

  local remove_data="n"
  if [ -z "${ACPMS_NONINTERACTIVE:-}" ]; then
    ask_yes "Also remove Docker data (Postgres, MinIO)? This deletes all DB and uploaded files. [y/N]" "n" && remove_data="y"
  else
    [ -n "${ACPMS_REMOVE_DATA:-}" ] && remove_data="y"
  fi
  if [ "$remove_data" = "y" ]; then
    log "Stopping containers and removing volumes..."
    docker stop acpms-postgres acpms-minio 2>/dev/null || true
    docker rm -f acpms-postgres acpms-minio 2>/dev/null || true
    if [ -f "$CONF_DIR/docker-compose.yml" ]; then
      (cd "$CONF_DIR" && $DOCKER_COMPOSE_CMD -p acpms down -v 2>/dev/null) || true
    fi
    docker volume rm acpms_postgres_data acpms_minio_data 2>/dev/null || true
    log "Docker data (Postgres, MinIO) removed."
  else
    log "Docker data (Postgres, MinIO) was NOT removed. Reinstall will reuse existing data."
  fi

  $USE_SUDO rm -rf "$BASE_DIR" "$CONF_DIR"
  if [ "$OS" = "linux" ]; then
    $USE_SUDO rm -rf "$WORK_DIR"
  fi
  log "Uninstall complete."
}

# =============================================================================
# Main
# =============================================================================

main_install() {
  log "ACPMS Installer - $REPO"
  ACPMS_SERVER_STARTED=0
  if [ -z "${ACPMS_NONINTERACTIVE:-}" ]; then
    log "This script will check prerequisites (curl, jq, tar, Docker, Docker Compose, cloudflared, Node.js) and may install them if missing."
    log "On Linux you may be prompted for your password (sudo) to install packages and run the server."
    ask_yes "Continue? [Y/n]" "y" || exit 0
  fi
  check_deps
  check_docker
  check_docker_compose
  detect_platform
  check_cloudflared
  check_services
  check_agent_cli_providers

  ACPMS_PORT=$(find_free_port)
  log "Using port: $ACPMS_PORT"
  prompt_public_url
  prompt_openclaw_gateway

  if [ -z "${ACPMS_NONINTERACTIVE:-}" ]; then
    log "S3 public endpoint will be: $(resolve_s3_public_base)/s3"
    log "ACPMS will be installed to: $BASE_DIR (config: $CONF_DIR)."
    ask_yes "Continue with download and install? [Y/n]" "y" || exit 0
  fi

  download_artifacts
  generate_env
  run_migration
  create_admin

  if [ "$OS" = "linux" ]; then
    setup_linux_daemon
  elif [ "$OS" = "darwin" ]; then
    setup_macos_daemon
  fi

  generate_openclaw_bootstrap_prompt_file
  unset ACPMS_ADMIN_LOGIN_PASSWORD

  # Final summary: banner + report
  print_success_banner
  print_success_report
}

# Parse --uninstall (optionally --yes / -y to skip confirmation)
UNINSTALL_CONFIRMED=
for arg in "$@"; do
  case "$arg" in
    --uninstall)
      ;;
    --yes|-y)
      UNINSTALL_CONFIRMED=1
      ;;
  esac
done
for arg in "$@"; do
  case "$arg" in
    --uninstall)
      [ -n "$UNINSTALL_CONFIRMED" ] && export ACPMS_NONINTERACTIVE=1
      detect_platform
      do_uninstall
      exit 0
      ;;
  esac
done

main_install
