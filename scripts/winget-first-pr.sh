#!/usr/bin/env bash
# Première soumission de untypequicode.Looty à microsoft/winget-pkgs : winget-first-pr.sh [version] [-y]
# À lancer par le mainteneur (compte GitHub connecté avec « gh auth login »). Demande confirmation avant chaque
# action sur GitHub : fork de microsoft/winget-pkgs, branche, push, PR. Les manifestes viennent de
# packaging/winget (ou sont régénérés si la version demandée n'y est pas : node scripts/winget-manifest.mjs).
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
version=""; yes=0
for a in "$@"; do case "$a" in -y) yes=1 ;; *) version="$a" ;; esac; done
[ -n "$version" ] || version="$(git -C "$root" tag -l 'desktop-v*' | sed 's/^desktop-v//' | sort -V | tail -1)"
rel="manifests/u/untypequicode/Looty/$version"
src="$root/packaging/winget/$rel"
if [ ! -d "$src" ]; then
  echo "Generating the manifests for $version (downloads the released installer)..."
  node "$root/scripts/winget-manifest.mjs" "$version" "$root/packaging/winget" --download
fi
ls "$src"/*.yaml > /dev/null
confirm() { # message
  [ "$yes" = 1 ] && return 0
  read -r -p "$1 [y/N] " r; [ "$r" = y ] || [ "$r" = Y ] || { echo "Cancelled."; exit 1; }
}
command -v gh > /dev/null || { echo "gh is required" >&2; exit 1; }
me="$(gh api user -q .login)"
echo "GitHub account: $me. Package untypequicode.Looty $version (installer: $(grep -m1 InstallerUrl "$src"/*.installer.yaml | sed 's/.*InstallerUrl: //'))"
confirm "Fork microsoft/winget-pkgs to $me and open a pull request?"
gh repo fork microsoft/winget-pkgs --clone=false
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
git clone --quiet --depth 1 --filter=blob:none --sparse "https://github.com/$me/winget-pkgs.git" "$work/w"
cd "$work/w"
git remote add upstream https://github.com/microsoft/winget-pkgs.git
git fetch --quiet --depth 1 upstream master
git checkout --quiet -B "untypequicode.Looty-$version" FETCH_HEAD
git sparse-checkout set "manifests/u/untypequicode"
mkdir -p "$rel"; cp "$src"/*.yaml "$rel/"
git add "$rel"
git commit --quiet -m "New package: untypequicode.Looty version $version"
confirm "Push branch untypequicode.Looty-$version to your fork and create the PR?"
git push --quiet -u origin "untypequicode.Looty-$version"
gh pr create --repo microsoft/winget-pkgs --head "$me:untypequicode.Looty-$version" --base master \
  --title "New package: untypequicode.Looty version $version" \
  --body "Adds Looty (self-hosted team workspace desktop client), NSIS installer per user, silent switch /S. Source: https://github.com/untypequicode/looty"
