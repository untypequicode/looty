#!/usr/bin/env bash
# Construit desktop/dist/looty_<version>_<arch>.deb avec Docker (Debian 12).
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
repo="$(cd "$here/.." && pwd)"
docker build -t looty-deb-builder -f "$here/packaging/Dockerfile.deb" "$here/packaging"
docker run --rm \
  -v "$repo:/src:ro" \
  -v looty-deb-cargo:/usr/local/cargo/registry \
  -v looty-deb-target:/target \
  -v "$here/dist:/dist" \
  looty-deb-builder \
  bash -c 'rm -rf /work && cp -a /src/desktop /work && cd /work/src-tauri && cargo tauri build --bundles deb && cp /target/release/bundle/deb/*.deb /dist/ && chown '"$(id -u):$(id -g)"' /dist/*.deb'
ls -lh "$here/dist/"*.deb
