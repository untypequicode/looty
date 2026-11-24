#!/usr/bin/env bash
# Prépare la construction Flatpak : sources Cargo hors ligne (cargo-sources.json), modules partagés Flathub,
# métadonnées AppStream. Résultat dans packaging/flatpak/ (fichiers non versionnés, voir .gitignore).
#   ./scripts/flatpak-prepare.sh
# Prérequis : git, node, python3 avec les modules « aiohttp », « tomlkit » et « PyYAML » (pour flatpak-cargo-generator).
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
out="$root/packaging/flatpak"
# Révisions épinglées (changer ici pour monter de version).
TOOLS_REF="${FLATPAK_BUILDER_TOOLS_REF:-9d86d336a39b9af38b86f114b6ce5820fe260f91}"
SHARED_REF="${FLATHUB_SHARED_MODULES_REF:-34add58f536223b780a3c9f077980cdaa51fc81f}"
fetch() { # dépôt, révision, dossier
  [ -d "$3/.git" ] || git clone --quiet "$1" "$3"
  git -C "$3" fetch --quiet origin "$2" 2> /dev/null || true
  git -C "$3" checkout --quiet "$2"
}
tmp="${FLATPAK_TMP:-${TMPDIR:-/tmp}/looty-flatpak}"
mkdir -p "$tmp"
fetch https://github.com/flathub/shared-modules.git "$SHARED_REF" "$out/shared-modules"
fetch https://github.com/flatpak/flatpak-builder-tools.git "$TOOLS_REF" "$tmp/flatpak-builder-tools"
python3 "$tmp/flatpak-builder-tools/cargo/flatpak-cargo-generator.py" "$root/desktop/src-tauri/Cargo.lock" -o "$out/cargo-sources.json"
node "$root/scripts/flatpak-render.mjs"
echo "Ready: cd $out && flatpak-builder --user --install-deps-from=flathub --force-clean build io.github.untypequicode.looty.yml"
