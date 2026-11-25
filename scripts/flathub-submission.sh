#!/usr/bin/env bash
# Prépare le dossier à soumettre à Flathub pour un tag desktop-vX.Y.Z : flathub-submission.sh [tag] [dossier]
#   tag      défaut : le dernier desktop-v* ; dossier de sortie défaut : /tmp/looty-flathub-submission
# Construit tout à partir du tag lui-même (Cargo.lock, notes de version) dans un worktree temporaire, puis
# écrit dans le dossier : manifeste Flathub, cargo-sources.json, metainfo.xml, flathub.json (x86_64 seulement).
# Prérequis : git, node, python3 avec aiohttp, tomlkit et PyYAML. Ne soumet rien (voir docs/packaging-flathub-winget.md).
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
git -C "$root" fetch --quiet --tags origin
tag="${1:-$(git -C "$root" tag -l 'desktop-v*' | sort -V | tail -1)}"
out="${2:-/tmp/looty-flathub-submission}"
[ -n "$tag" ] || { echo "no desktop-v* tag" >&2; exit 1; }
tmp="$(mktemp -d)"; trap 'git -C "$root" worktree remove --force "$tmp/src" 2> /dev/null || true; rm -rf "$tmp"' EXIT
git -C "$root" worktree add --quiet --detach "$tmp/src" "$tag"
FLATPAK_TMP="$tmp/tools" "$tmp/src/scripts/flatpak-prepare.sh" > /dev/null
rm -rf "$out"; mkdir -p "$out"
node "$tmp/src/scripts/flatpak-render.mjs" --flathub "$out" > /dev/null
cp "$tmp/src/packaging/flatpak/cargo-sources.json" "$tmp/src/packaging/flatpak/io.github.untypequicode.looty.metainfo.xml" "$out/"
printf '{\n  "only-arches": ["x86_64"]\n}\n' > "$out/flathub.json"
echo "Flathub submission for $tag written to $out:"; ls -1 "$out"
