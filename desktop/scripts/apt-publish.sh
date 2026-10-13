#!/usr/bin/env bash
# Construit le dépôt apt signé d'après un .deb et le dépose sur le site : apt-publish.sh <paquet.deb>
# (clé de signature déjà importée dans gpg ; GPG_PASSPHRASE si elle est protégée.) Seule la dernière
# version reste dans le dépôt : apt n'en propose pas d'autre.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; root="$(cd "$here/../.." && pwd)"
deb="${1:?paquet .deb}"
out="$(mktemp -d)"; trap 'rm -rf "$out"' EXIT
"$here/apt-repo.sh" "$out/apt" "$deb"
(cd "$out" && find apt -type f) | while read -r f; do "$root/scripts/site-upload.sh" "$out/$f" "$f"; done
pkg="$(ls "$out/apt/pool/main")"
"$root/scripts/site-delete.sh" apt/pool/main $pkg
