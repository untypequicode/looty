#!/usr/bin/env bash
# Publie le dépôt apt signé sur GitHub Pages (branche gh-pages, dossier apt/) : apt-publish-pages.sh <paquet.deb>
# Pour un dépôt public, sans site de téléchargement. Variables : GH_TOKEN, GITHUB_REPOSITORY (fournies par GitHub Actions) ;
# clé de signature déjà importée dans gpg. Seule la dernière version reste dans le dépôt. Pages doit être activé
# (Settings → Pages → branche gh-pages).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
deb="${1:?paquet .deb}"
pages="$(mktemp -d)"; trap 'rm -rf "$pages"' EXIT
url="https://x-access-token:${GH_TOKEN:?GH_TOKEN}@github.com/${GITHUB_REPOSITORY:?GITHUB_REPOSITORY}.git"
if git ls-remote --exit-code --heads "$url" gh-pages > /dev/null 2>&1; then
  git clone -q --depth 1 --branch gh-pages "$url" "$pages"
else
  git -C "$pages" init -q -b gh-pages && git -C "$pages" remote add origin "$url"
fi
rm -rf "$pages/apt/pool"
"$here/apt-repo.sh" "$pages/apt" "$deb"
touch "$pages/.nojekyll"
git -C "$pages" add -A
git -C "$pages" -c user.name=looty-ci -c user.email=ci@users.noreply.github.com commit -q -m "apt: $(basename "$deb")" || { echo "dépôt apt inchangé"; exit 0; }
git -C "$pages" push -q origin gh-pages
echo "dépôt apt publié sur GitHub Pages"
