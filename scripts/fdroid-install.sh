#!/usr/bin/env bash
# Installe fdroidserver dans un environnement Python à part (CI) et l'ajoute au PATH des étapes suivantes.
# Le paquet apt d'Ubuntu 24.04 (2.2.1) lit mal les ressources des APK récents : on prend la version de PyPI.
# FDROIDSERVER_VERSION (facultatif) fige la version, par exemple FDROIDSERVER_VERSION=2.4.3.
set -euo pipefail
venv="${RUNNER_TEMP:-/tmp}/fdroid-venv"
python3 -m venv "$venv"
"$venv/bin/pip" install --quiet --disable-pip-version-check "fdroidserver${FDROIDSERVER_VERSION:+==$FDROIDSERVER_VERSION}"
"$venv/bin/pip" show fdroidserver | sed -n 's/^Version: /fdroidserver /p'
[ -z "${GITHUB_PATH:-}" ] || echo "$venv/bin" >> "$GITHUB_PATH"
