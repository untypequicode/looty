#!/usr/bin/env bash
# Crée la clé de signature du dépôt apt (à faire une seule fois, par le mainteneur).
#   - desktop/packaging/looty-archive-keyring.gpg : clé PUBLIQUE, à ajouter au dépôt git (elle part dans le .deb)
#   - clé PRIVÉE : à copier dans le secret GitHub APT_GPG_PRIVATE_KEY (et APT_GPG_PASSPHRASE si protégée)
# gpg manquant (shell Nix, système minimal) : relance le script avec gnupg fourni par Nix.
if ! command -v gpg >/dev/null 2>&1; then
  if command -v nix >/dev/null 2>&1; then exec nix shell nixpkgs#gnupg -c "$0" "$@"; fi
  echo "gpg introuvable : installez gnupg (sudo apt install gnupg)" >&2; exit 1
fi
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
home="$(mktemp -d)"; trap 'rm -rf "$home"' EXIT
export GNUPGHOME="$home"; chmod 700 "$home"
read -r -p "Nom (ex. Looty APT) : " name
read -r -p "E-mail : " email
read -r -s -p "Phrase secrète (vide = aucune ; sinon la copier aussi dans APT_GPG_PASSPHRASE) : " pass; echo
gpg --batch --pinentry-mode loopback --passphrase "$pass" --quick-generate-key "$name <$email>" ed25519 sign never
gpg --export > "$here/packaging/looty-archive-keyring.gpg"
echo
echo "Clé publique écrite : desktop/packaging/looty-archive-keyring.gpg  (à commiter)"
echo
echo "Secret GitHub APT_GPG_PRIVATE_KEY (copiez tout, de BEGIN à END) :"
gpg --batch --pinentry-mode loopback --passphrase "$pass" --armor --export-secret-keys
