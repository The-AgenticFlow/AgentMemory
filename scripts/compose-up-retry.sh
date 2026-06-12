#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
read -r -a COMPOSE_CMD <<< "${COMPOSE_CMD:-docker compose}"
MAX_PULL_ATTEMPTS="${MAX_PULL_ATTEMPTS:-5}"
INITIAL_DELAY_SECONDS="${INITIAL_DELAY_SECONDS:-3}"

retry_pull() {
  local service="$1"
  local attempt=1
  local delay="$INITIAL_DELAY_SECONDS"

  while true; do
    echo "Pulling ${service} (attempt ${attempt}/${MAX_PULL_ATTEMPTS})..."
    if "${COMPOSE_CMD[@]}" pull "${service}"; then
      return 0
    fi

    if (( attempt >= MAX_PULL_ATTEMPTS )); then
      echo "Failed to pull ${service} after ${MAX_PULL_ATTEMPTS} attempts." >&2
      return 1
    fi

    echo "Pull failed because of a transient network issue; retrying in ${delay}s..." >&2
    sleep "${delay}"
    attempt=$((attempt + 1))
    delay=$((delay * 2))
  done
}

cd "${ROOT_DIR}"

retry_pull neo4j
"${COMPOSE_CMD[@]}" up --build
