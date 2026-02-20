#!/usr/bin/env bash
set -euo pipefail

ARTIFACT_ROOT="${1:-artifacts}"
OUTPUT_PATH="${2:-worker-manifest.json}"
CHANNEL="${3:-stable}"
VERSION="${4:-0.0.0}"
TAG="${5:-v0.0.0}"
REPO="${6:-owner/repo}"
PUBLISHED_AT="${7:-$(date -u +"%Y-%m-%dT%H:%M:%SZ")}"

entries=()

if [[ -f "${ARTIFACT_ROOT}/darkreach-worker-linux-x86_64/darkreach-worker-linux-x86_64.tar.gz" ]]; then
  hash="$(cut -d' ' -f1 "${ARTIFACT_ROOT}/darkreach-worker-linux-x86_64/darkreach-worker-linux-x86_64.tar.gz.sha256")"
  url="https://github.com/${REPO}/releases/download/${TAG}/darkreach-worker-linux-x86_64.tar.gz"
  sig_url=""
  if [[ -f "${ARTIFACT_ROOT}/darkreach-worker-linux-x86_64/darkreach-worker-linux-x86_64.tar.gz.sig" ]]; then
    sig_url="https://github.com/${REPO}/releases/download/${TAG}/darkreach-worker-linux-x86_64.tar.gz.sig"
  fi
  if [[ -n "${sig_url}" ]]; then
    entries+=("{\"os\":\"linux\",\"arch\":\"x86_64\",\"url\":\"${url}\",\"sha256\":\"${hash}\",\"sig_url\":\"${sig_url}\"}")
  else
    entries+=("{\"os\":\"linux\",\"arch\":\"x86_64\",\"url\":\"${url}\",\"sha256\":\"${hash}\"}")
  fi
fi

if [[ -f "${ARTIFACT_ROOT}/darkreach-worker-linux-aarch64/darkreach-worker-linux-aarch64.tar.gz" ]]; then
  hash="$(cut -d' ' -f1 "${ARTIFACT_ROOT}/darkreach-worker-linux-aarch64/darkreach-worker-linux-aarch64.tar.gz.sha256")"
  url="https://github.com/${REPO}/releases/download/${TAG}/darkreach-worker-linux-aarch64.tar.gz"
  sig_url=""
  if [[ -f "${ARTIFACT_ROOT}/darkreach-worker-linux-aarch64/darkreach-worker-linux-aarch64.tar.gz.sig" ]]; then
    sig_url="https://github.com/${REPO}/releases/download/${TAG}/darkreach-worker-linux-aarch64.tar.gz.sig"
  fi
  if [[ -n "${sig_url}" ]]; then
    entries+=("{\"os\":\"linux\",\"arch\":\"aarch64\",\"url\":\"${url}\",\"sha256\":\"${hash}\",\"sig_url\":\"${sig_url}\"}")
  else
    entries+=("{\"os\":\"linux\",\"arch\":\"aarch64\",\"url\":\"${url}\",\"sha256\":\"${hash}\"}")
  fi
fi

if [[ "${#entries[@]}" -eq 0 ]]; then
  echo "No release artifacts found under ${ARTIFACT_ROOT}" >&2
  exit 1
fi

artifacts_json="$(IFS=,; echo "${entries[*]}")"

cat >"${OUTPUT_PATH}" <<EOF
{
  "channels": {
    "${CHANNEL}": {
      "version": "${VERSION}",
      "published_at": "${PUBLISHED_AT}",
      "notes": "Generated from GitHub release ${TAG}",
      "artifacts": [${artifacts_json}]
    }
  }
}
EOF

echo "Wrote ${OUTPUT_PATH}"
