#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <remote_dashboard_base_url>"
  echo "Example: $0 https://primehunt.example.com"
  exit 1
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REMOTE_BASE="${1%/}"

if [[ "$REMOTE_BASE" =~ ^https:// ]]; then
  REMOTE_WS_URL="wss://${REMOTE_BASE#https://}/ws"
elif [[ "$REMOTE_BASE" =~ ^http:// ]]; then
  REMOTE_WS_URL="ws://${REMOTE_BASE#http://}/ws"
else
  echo "Remote URL must start with http:// or https://"
  exit 1
fi

echo "Starting local frontend on :3000 using remote data source:"
echo "  API proxy target: $REMOTE_BASE"
echo "  WS direct target: $REMOTE_WS_URL"

cd "$ROOT/frontend"
DEV_PROXY_TARGET="$REMOTE_BASE" \
NEXT_PUBLIC_API_URL="" \
NEXT_PUBLIC_WS_URL="$REMOTE_WS_URL" \
npm run dev -- --port 3000
