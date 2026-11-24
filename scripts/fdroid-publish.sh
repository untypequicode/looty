#!/usr/bin/env bash
# Publie le dépôt F-Droid construit par fdroid-repo.sh sur le site : fdroid-publish.sh <dossier-de-sortie>
# (SITE_URL et SITE_UPLOAD_TOKEN ; sans eux, rien n'est déposé.) Ordre : fichiers de données (APK, icônes), puis
# l'index, puis retrait des anciens fichiers (dernières versions seulement, comme releases/).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
out="$(realpath "${1:?dossier de sortie}")"
[ -d "$out/repo" ] || { echo "fdroid-publish: $out/repo not found" >&2; exit 1; }
index='^(index.*|entry.*)$'
cd "$out/repo"
# 1. Tout sauf les fichiers d'index.
find . -type f | sed 's|^\./||' | sort | while read -r f; do
  base="$(basename "$f")"
  if [[ "$base" =~ $index ]] && [[ "$f" != */* ]]; then continue; fi
  "$here/site-upload.sh" "$f" "fdroid/repo/$f"
done
# 2. Index (les clients lisent entry.jar/index-v2.json en dernier : ils ne voient jamais un index sans son APK).
for f in $(find . -maxdepth 1 -type f | sed 's|^\./||' | sort | grep -E "$index" || true); do
  "$here/site-upload.sh" "$f" "fdroid/repo/$f"
done
"$here/site-upload.sh" "$out/fingerprint.txt" fdroid/fingerprint.txt
# 3. Anciens fichiers retirés, dossier par dossier.
for d in $(find . -type d | sed 's|^\./\?||' | sort); do
  keep=$(find "./$d" -maxdepth 1 -type f -exec basename {} \;)
  # shellcheck disable=SC2086
  "$here/site-delete.sh" "fdroid/repo${d:+/$d}" $keep
done
