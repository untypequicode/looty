#!/usr/bin/env bash
# Régénère les images Fastlane (icône 512x512, image de présentation 1024x500, captures téléphone) à partir de
# desktop/src-tauri/icons/icon.png et des captures du site (site/web/public/shots/phone-*-light-<langue>.webp).
# Les PNG sont versionnés ; relancer ce script seulement si les captures ou l'icône changent. Nécessite ffmpeg.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
shots="$root/site/web/public/shots"
for pair in en:en-US fr:fr-FR; do
  l="${pair%%:*}"; loc="${pair##*:}"
  dir="$root/fastlane/metadata/android/$loc/images"
  rm -rf "$dir"; mkdir -p "$dir/phoneScreenshots"
  ffmpeg -v error -y -i "$root/desktop/src-tauri/icons/icon.png" -vf scale=512:512 "$dir/icon.png"
  n=1
  for s in home tasks board doc; do
    ffmpeg -v error -y -i "$shots/phone-$s-light-$l.webp" -vf scale=540:1200 "$dir/phoneScreenshots/$n.png"
    n=$((n + 1))
  done
  ffmpeg -v error -y -f lavfi -i "color=c=0x5440c8:s=1024x500" -i "$root/desktop/src-tauri/icons/icon.png" \
    -i "$shots/phone-home-light-$l.webp" -i "$shots/phone-board-light-$l.webp" -filter_complex \
    "[1]scale=240:240[i];[2]scale=-1:420[a];[3]scale=-1:420[b];[0][i]overlay=60:130[x];[x][a]overlay=420:40[y];[y][b]overlay=720:80" \
    -frames:v 1 "$dir/featureGraphic.png"
done
