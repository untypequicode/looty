#!/usr/bin/env bash
# Prépare une version de l'application de bureau : ./desktop/scripts/release.sh 0.2.0
# Met à jour Cargo.toml, Cargo.lock et tauri.conf.json, puis indique les commandes git.
set -euo pipefail
v="${1:?usage: release.sh X.Y.Z}"
[[ "$v" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "version must look like 1.2.3"; exit 1; }
here="$(cd "$(dirname "$0")/.." && pwd)"
sed -i "0,/^version = \".*\"/s//version = \"$v\"/" "$here/src-tauri/Cargo.toml"
sed -i "0,/\"version\": \".*\"/s//\"version\": \"$v\"/" "$here/src-tauri/tauri.conf.json"
(cd "$here/src-tauri" && cargo update -p looty-desktop --offline >/dev/null 2>&1 || true)
echo "Version set to $v. Next:"
echo "  git commit -am \"Desktop $v\" && git tag desktop-v$v && git push --follow-tags"
