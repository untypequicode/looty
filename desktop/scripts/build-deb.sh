#!/usr/bin/env bash
# Construit desktop/dist/looty_<version>_<arch>.deb avec Docker (Debian 12).
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
repo="$(cd "$here/.." && pwd)"
# LOOTY_DEB_SKIP_IMAGE=1 : image déjà construite (CI, avec cache des couches).
[ "${LOOTY_DEB_SKIP_IMAGE:-}" = 1 ] || docker build -t looty-deb-builder -f "$here/packaging/Dockerfile.deb" "$here/packaging"
# Caches cargo (dépendances téléchargées et compilées) : volumes Docker, ou dossier de
# l'hôte avec LOOTY_DEB_CACHE (CI : conservé entre deux exécutions par actions/cache).
if [ -n "${LOOTY_DEB_CACHE:-}" ]; then
  mkdir -p "$LOOTY_DEB_CACHE/registry" "$LOOTY_DEB_CACHE/target"
  caches=(-v "$LOOTY_DEB_CACHE/registry:/usr/local/cargo/registry" -v "$LOOTY_DEB_CACHE/target:/target")
else
  caches=(-v looty-deb-cargo:/usr/local/cargo/registry -v looty-deb-target:/target)
fi
docker run --rm \
  -v "$repo:/src:ro" \
  "${caches[@]}" \
  -v "$here/dist:/dist" \
  looty-deb-builder \
  bash -c 'rm -rf /work && cp -a /src/desktop /work && cd /work/src-tauri && cargo tauri build --bundles deb && cp /target/release/bundle/deb/*.deb /dist/ && chown '"$(id -u):$(id -g)"' /dist/*.deb'
ls -lh "$here/dist/"*.deb
