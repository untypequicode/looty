#!/usr/bin/env bash
# Retire du site les fichiers d'un dossier distant qui ne sont pas dans la liste :
#   site-delete.sh <dossier-distant> <fichier-à-garder>...
set -euo pipefail
dir="${1:?dossier}"; shift
[ -n "${SITE_URL:-}" ] && [ -n "${SITE_UPLOAD_TOKEN:-}" ] || exit 0
names=$(curl --fail --silent --show-error -H "Authorization: Bearer $SITE_UPLOAD_TOKEN" "${SITE_URL%/}/_ls/$dir") || exit 0
keep=" $* "
for n in $(node -e 'for (const n of JSON.parse(process.argv[1])) console.log(n)' "$names"); do
  case "$keep" in *" $n "*) ;; *) curl --fail --silent --show-error -X DELETE -H "Authorization: Bearer $SITE_UPLOAD_TOKEN" "${SITE_URL%/}/_upload/$dir/$n" && echo "site : $dir/$n retiré" ;; esac
done
