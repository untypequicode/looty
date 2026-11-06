#!/usr/bin/env bash
# Prune the backend's Cargo build cache. Cargo never removes old incremental
# sessions or outdated dependency builds, so target/ grows without bound.
# Usage: scripts/clean-target.sh [max_gb]   (default 8: wipes target/ above that size)
set -euo pipefail
cd "$(dirname "$0")/../backend"
max_gb="${1:-8}"
[ -d target ] || exit 0
size_gb=$(( $(du -sk target | cut -f1) / 1024 / 1024 ))
if [ "$size_gb" -ge "$max_gb" ]; then
  echo "backend/target is ${size_gb} GB (>= ${max_gb}): removing"
  rm -rf target
else
  echo "backend/target is ${size_gb} GB: fine"
fi
