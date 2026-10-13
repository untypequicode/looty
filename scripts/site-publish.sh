#!/usr/bin/env bash
# Publie une version sur le site de téléchargement : site-publish.sh <android|linux|windows> <fichier>
# Dépose le fichier, puis sa fiche (latest.json, qui fait foi pour /latest.json et /download/…), puis retire les anciens fichiers.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
platform="${1:?plateforme}"; file="${2:?fichier}"
name="$(basename "$file")"
meta="$(mktemp)"; trap 'rm -f "$meta"' EXIT
node "$here/site-release-meta.mjs" "$platform" "$file" > "$meta"
"$here/site-upload.sh" "$file" "releases/$platform/$name"
"$here/site-upload.sh" "$meta" "releases/$platform/latest.json"
"$here/site-delete.sh" "releases/$platform" "$name" latest.json
