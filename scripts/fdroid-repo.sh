#!/usr/bin/env bash
# Fabrique le dépôt F-Droid de Looty : fdroid-repo.sh <apk-fdroid-non-signé> <dossier-de-sortie>
# Signe l'APK avec la clé de l'application (même clé que la variante « direct »), puis lance « fdroid update »
# (index signé avec la clé du dépôt). Seule la version donnée est dans le dépôt (les anciennes disparaissent).
# Secrets lus dans l'environnement (jamais écrits dans un fichier du dépôt) :
#   APK_KEYSTORE (chemin), APK_KEYSTORE_PASSWORD, APK_KEY_ALIAS, APK_KEY_PASSWORD   clé de signature de l'APK
#   REPO_KEYSTORE (chemin), REPO_KEYSTORE_PASSWORD, REPO_KEY_ALIAS, REPO_KEY_PASSWORD   clé de l'index du dépôt
# Facultatif : FDROID_REPO_URL (défaut https://looty.untypequicode.fr/fdroid/repo).
# Résultat : <sortie>/repo/… (à publier tel quel) et <sortie>/fingerprint.txt (empreinte de la clé du dépôt).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; root="$(cd "$here/.." && pwd)"
unsigned="$(realpath "${1:?apk non signé}")"; out="${2:?dossier de sortie}"
for v in APK_KEYSTORE APK_KEYSTORE_PASSWORD APK_KEY_ALIAS APK_KEY_PASSWORD REPO_KEYSTORE REPO_KEYSTORE_PASSWORD REPO_KEY_ALIAS REPO_KEY_PASSWORD; do
  [ -n "${!v:-}" ] || { echo "fdroid-repo: $v is not set" >&2; exit 1; }
done
appid=io.github.untypequicode.looty
mkdir -p "$out"; out="$(realpath "$out")"
rm -rf "$out/repo" "$out/metadata" "$out/config.yml"
mkdir -p "$out/repo" "$out/metadata/$appid"

sdk="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}"
apksigner="$(ls "$sdk"/build-tools/*/apksigner 2> /dev/null | sort -V | tail -1 || true)"
[ -n "$apksigner" ] || apksigner="$(command -v apksigner || true)"
[ -n "$apksigner" ] || { echo "fdroid-repo: apksigner not found (ANDROID_HOME?)" >&2; exit 1; }

# 1. APK signé, nom attendu par F-Droid : <identifiant>_<versionCode>.apk
code="$(grep -m1 'versionCode =' "$root/android/app/build.gradle.kts" | grep -o '[0-9]\+')"
APK_KS_PASS="$APK_KEYSTORE_PASSWORD" APK_K_PASS="$APK_KEY_PASSWORD" "$apksigner" sign \
  --ks "$APK_KEYSTORE" --ks-key-alias "$APK_KEY_ALIAS" --ks-pass env:APK_KS_PASS --key-pass env:APK_K_PASS \
  --out "$out/repo/${appid}_${code}.apk" "$unsigned"
"$apksigner" verify "$out/repo/${appid}_${code}.apk"

# 2. Fiche et métadonnées Fastlane (titre, descriptions, journal des changements)
cp "$here/fdroid/$appid.yml" "$out/metadata/$appid.yml"
for loc in "$root"/fastlane/metadata/android/*/; do
  cp -r "$loc" "$out/metadata/$appid/$(basename "$loc")"
done

# 3. Configuration de F-Droid (dossier de travail temporaire, droits 600, supprimée ensuite). Les mots de passe
#    sont écrits en chaîne JSON, valide en YAML, quels que soient leurs caractères.
yq() { node -e 'process.stdout.write(JSON.stringify(process.env[process.argv[1]]))' "$1"; }
umask 077
cat > "$out/config.yml" <<EOF
repo_url: ${FDROID_REPO_URL:-https://looty.untypequicode.fr/fdroid/repo}
repo_name: Looty
repo_description: Looty releases without a built-in updater, signed by the Looty project.
repo_keyalias: $(yq REPO_KEY_ALIAS)
keystore: $(realpath "$REPO_KEYSTORE")
keystorepass: $(yq REPO_KEYSTORE_PASSWORD)
keypass: $(yq REPO_KEY_PASSWORD)
archive_older: 0
make_current_version_link: false
sdk_path: $sdk
EOF
chmod 600 "$out/config.yml"
trap 'rm -f "$out/config.yml"' EXIT
( cd "$out" && fdroid update )
rm -f "$out/config.yml"

# 4. Empreinte de la clé du dépôt (SHA-256 du certificat, majuscules, sans « : »)
keytool -list -v -keystore "$REPO_KEYSTORE" -storepass "$REPO_KEYSTORE_PASSWORD" -alias "$REPO_KEY_ALIAS" \
  | sed -n 's/^ *SHA256: *//p' | head -1 | tr -d ':' | tr 'a-f' 'A-F' > "$out/fingerprint.txt"
[ -s "$out/fingerprint.txt" ] || { echo "fdroid-repo: no fingerprint" >&2; exit 1; }
test -f "$out/repo/index-v1.json" && grep -q "\"$appid\"" "$out/repo/index-v1.json"
echo "fdroid repository ready in $out/repo (key fingerprint $(cat "$out/fingerprint.txt"))"
