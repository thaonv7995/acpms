#!/usr/bin/env bash
# Verify CLI provider command resolution with a production-like PATH.
# Resolution order per provider:
#   1) ACPMS_AGENT_<PROVIDER>_BIN override (absolute executable path)
#   2) provider binary in PATH
#   3) npx fallback for npm providers (claude/codex/gemini)

set -euo pipefail

CHECK_PATH="${PATH:-/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin}"

usage() {
    echo "Usage: $0 [--path <PATH>]"
    echo "  --path <PATH>  Override PATH used for provider checks"
}

while [ $# -gt 0 ]; do
    case "$1" in
        --path)
            if [ $# -lt 2 ]; then
                echo "[ERROR] --path requires a value" >&2
                exit 1
            fi
            CHECK_PATH="$2"
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "[ERROR] Unknown option: $1" >&2
            usage
            exit 1
            ;;
    esac
done

has_bin() {
    PATH="$CHECK_PATH" command -v "$1" >/dev/null 2>&1
}

resolve_bin() {
    PATH="$CHECK_PATH" command -v "$1" 2>/dev/null || true
}

check_provider() {
    local provider_label="$1"
    local override_var="$2"
    local primary_bin="$3"
    local npm_package="${4:-}"

    local override="${!override_var:-}"
    if [ -n "$override" ]; then
        if [ -x "$override" ]; then
            echo "[OK]   $provider_label via $override_var=$override"
            return 0
        fi
        echo "[FAIL] $provider_label override $override_var is set but not executable: $override"
        return 1
    fi

    if has_bin "$primary_bin"; then
        local resolved_primary
        resolved_primary="$(resolve_bin "$primary_bin")"
        echo "[OK]   $provider_label via PATH binary '$primary_bin' ($resolved_primary)"
        return 0
    fi

    if [ -n "$npm_package" ] && has_bin "npx"; then
        local resolved_npx
        resolved_npx="$(resolve_bin "npx")"
        echo "[OK]   $provider_label via npx fallback '$npm_package' ($resolved_npx)"
        return 0
    fi

    if [ -n "$npm_package" ]; then
        echo "[FAIL] $provider_label missing '$primary_bin' and no 'npx' found in PATH"
    else
        echo "[FAIL] $provider_label missing '$primary_bin' in PATH"
    fi
    return 1
}

echo "[INFO] Provider smoke check PATH=$CHECK_PATH"

failures=0
check_provider "Claude Code" "ACPMS_AGENT_CLAUDE_BIN" "claude" "@anthropic-ai/claude-code" || failures=$((failures + 1))
check_provider "OpenAI Codex" "ACPMS_AGENT_CODEX_BIN" "codex" "@openai/codex" || failures=$((failures + 1))
check_provider "Gemini CLI" "ACPMS_AGENT_GEMINI_BIN" "gemini" "@google/gemini-cli" || failures=$((failures + 1))
check_provider "Cursor CLI" "ACPMS_AGENT_CURSOR_BIN" "agent" || failures=$((failures + 1))

if [ "$failures" -gt 0 ]; then
    echo "[WARN] Provider smoke check failed for $failures provider(s)"
    exit 1
fi

echo "[SUCCESS] Provider smoke check passed for all providers"
