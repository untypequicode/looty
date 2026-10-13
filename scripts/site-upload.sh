#!/usr/bin/env bash
# Dépose un fichier sur le site de téléchargement : site-upload.sh <fichier> <chemin-distant>
# (SITE_URL et SITE_UPLOAD_TOKEN : variable et secret du dépôt ; sans eux, le dépôt est simplement ignoré.)
set -euo pipefail
file="${1:?fichier}"; remote="${2:?chemin distant}"
if [ -z "${SITE_URL:-}" ] || [ -z "${SITE_UPLOAD_TOKEN:-}" ]; then echo "site : SITE_URL / SITE_UPLOAD_TOKEN absents, $remote ignoré"; exit 0; fi
curl --fail --silent --show-error --retry 3 --retry-delay 3 --max-time 1800 \
  -T "$file" -H "Authorization: Bearer $SITE_UPLOAD_TOKEN" "${SITE_URL%/}/_upload/$remote"
echo "site : $remote déposé"
