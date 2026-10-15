#!/usr/bin/env bash
# Construit desktop/dist/looty_<version>_<arch>.deb avec Docker (Debian 12).
# Si desktop/packaging/looty-archive-keyring.gpg existe (clé publique du dépôt apt), le paquet installe aussi
# le dépôt apt de Looty (/etc/apt/sources.list.d/looty.sources) : les mises à jour passent alors par apt.
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
# Dépôt apt : sur le site de téléchargement si LOOTY_SITE_URL est défini (la CI le fait avec la variable SITE_URL, comme
# pour la publication), sinon sur les GitHub Pages du dépôt public (https://<propriétaire>.github.io/<dépôt>).
site="${LOOTY_SITE_URL:-}"
[ -n "$site" ] || site="$(grep -m1 '"repository"' "$repo/backend/releases.json" | cut -d'"' -f4 | sed -E 's#https://github.com/([^/]+)/([^/]+)#https://\1.github.io/\2#')"
docker run --rm -e "LOOTY_SITE=${site%/}" \
  -v "$repo:/src:ro" \
  "${caches[@]}" \
  -v "$here/dist:/dist" \
  looty-deb-builder \
  bash -c 'rm -rf /work /target/release/bundle && cp -a /src/desktop /work && cd /work/src-tauri && \
    extra=() && if [ -f ../packaging/looty-archive-keyring.gpg ] && [ -n "$LOOTY_SITE" ]; then sed -i "s#@SITE@#$LOOTY_SITE#" ../packaging/looty.sources; extra=(--config '"'"'{"bundle":{"linux":{"deb":{"files":{"/etc/apt/sources.list.d/looty.sources":"../packaging/looty.sources","/usr/share/keyrings/looty-archive-keyring.gpg":"../packaging/looty-archive-keyring.gpg"}}}}}'"'"'); fi && \
    cargo tauri build --bundles deb "${extra[@]}" && cp /target/release/bundle/deb/*.deb /dist/ && chown '"$(id -u):$(id -g)"' /dist/*.deb'
ls -lh "$here/dist/"*.deb
