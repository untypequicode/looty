#!/usr/bin/env bash
# Construit (ou met à jour) un dépôt apt signé à partir de paquets .deb.
#
#   apt-repo.sh <dossier-du-dépôt> <paquet.deb>...
#
# Le dossier peut déjà contenir un dépôt (versions précédentes) : les nouveaux paquets s'y ajoutent
# et seules les 5 dernières versions sont gardées. Variables :
#   GPG_KEY_ID      clé de signature (par défaut : la première clé secrète du trousseau)
#   GPG_PASSPHRASE  phrase secrète de la clé, s'il y en a une
#   APT_SUITE       suite du dépôt (défaut : stable)
set -euo pipefail
out="${1:?dossier du dépôt}"; shift
[ "$#" -gt 0 ] || { echo "aucun paquet .deb" >&2; exit 1; }
suite="${APT_SUITE:-stable}"
mkdir -p "$out/pool/main" "$out/dists/$suite/main/binary-amd64"
# Nom de fichier normalisé d'après le paquet lui-même (Tauri nomme le fichier « Looty_… »).
for f in "$@"; do
  cp -f "$f" "$out/pool/main/$(dpkg-deb -f "$f" Package)_$(dpkg-deb -f "$f" Version)_$(dpkg-deb -f "$f" Architecture).deb"
done
# 5 versions au plus.
(cd "$out/pool/main" && ls -1 looty_*.deb 2>/dev/null | sort -V | head -n -5 | xargs -r rm -f)

cd "$out"
apt-ftparchive packages pool > "dists/$suite/main/binary-amd64/Packages"
gzip -9 -k -f "dists/$suite/main/binary-amd64/Packages"
apt-ftparchive \
  -o "APT::FTPArchive::Release::Origin=Looty" \
  -o "APT::FTPArchive::Release::Label=Looty" \
  -o "APT::FTPArchive::Release::Suite=$suite" \
  -o "APT::FTPArchive::Release::Codename=$suite" \
  -o "APT::FTPArchive::Release::Architectures=amd64" \
  -o "APT::FTPArchive::Release::Components=main" \
  release "dists/$suite" > "dists/$suite/Release"

gpg_args=(--batch --yes --pinentry-mode loopback)
[ -n "${GPG_PASSPHRASE:-}" ] && gpg_args+=(--passphrase "$GPG_PASSPHRASE")
key="${GPG_KEY_ID:-$(gpg --list-secret-keys --with-colons | awk -F: '/^sec/ {print $5; exit}')}"
[ -n "$key" ] || { echo "aucune clé de signature" >&2; exit 1; }
gpg "${gpg_args[@]}" --default-key "$key" --clearsign -o "dists/$suite/InRelease" "dists/$suite/Release"
gpg "${gpg_args[@]}" --default-key "$key" --armor --detach-sign -o "dists/$suite/Release.gpg" "dists/$suite/Release"
# Clé publique (pour ajouter le dépôt à la main).
gpg --export "$key" > looty-archive-keyring.gpg
gpg --armor --export "$key" > looty-archive-keyring.asc
echo "dépôt prêt dans $out ($(ls pool/main | wc -l) paquet(s))"
