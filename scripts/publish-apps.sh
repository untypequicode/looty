#!/usr/bin/env bash
# Publie les applications d'un dossier : publish-apps.sh <dossier>
# (workflows « Release » et « Site sync »). Dans le dossier, la version la plus récente de chaque fichier :
#   APK signé (jamais un APK de debug), installateur Windows, .deb → site de téléchargement (SITE_URL, SITE_UPLOAD_TOKEN ;
#   sans eux, ignoré) ;
#   .deb → dépôt apt signé si APT_GPG_PRIVATE_KEY est fourni (GPG_PASSPHRASE si la clé est protégée) : sur le site si
#   SITE_URL est défini (l'adresse que build-deb.sh inscrit dans le paquet), sinon sur GitHub Pages (GH_TOKEN, GITHUB_REPOSITORY).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; root="$(cd "$here/.." && pwd)"
dir="${1:?dossier}"
newest() { find "$dir" -maxdepth 1 -type f -name "$1" ! -name '*-debug*' | sort -V | tail -1; }
apk="$(newest 'looty-[0-9]*.apk')"; exe="$(newest '*-setup.exe')"; deb="$(newest '*.deb')"

if [ -n "${SITE_URL:-}" ] && [ -z "${SITE_UPLOAD_TOKEN:-}" ]; then
  echo "::warning title=Download site::SITE_URL is set but SITE_UPLOAD_TOKEN is missing: nothing is uploaded"
fi
[ -z "$apk" ] || "$here/site-publish.sh" android "$apk"
[ -z "$exe" ] || "$here/site-publish.sh" windows "$exe"
[ -z "$deb" ] || "$here/site-publish.sh" linux "$deb"

if [ -n "$deb" ] && [ -n "${APT_GPG_PRIVATE_KEY:-}" ]; then
  command -v apt-ftparchive > /dev/null || { sudo apt-get update -qq && sudo apt-get install -y -qq apt-utils gnupg; }
  printf '%s\n' "$APT_GPG_PRIVATE_KEY" | gpg --batch --import \
    || { echo "::error title=apt::APT_GPG_PRIVATE_KEY is not a valid armored key: paste the output of gpg --armor --export-secret-keys <fingerprint>, BEGIN and END lines included"; exit 1; }
  if [ -n "${SITE_URL:-}" ]; then "$root/desktop/scripts/apt-publish.sh" "$deb"; else "$root/desktop/scripts/apt-publish-pages.sh" "$deb"; fi
fi
