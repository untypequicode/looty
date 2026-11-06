#!/bin/sh
# Démarre la pile Caddy produite par deploy/render.sh sur https://localhost (certificat de l'AC interne de Caddy),
# lance deploy/test/smoke.mjs, puis arrête et supprime la pile.
#   LOOTY_VERSION=latest sh deploy/test/caddy-stack.sh
# LOOTY_VERSION choisit les images ghcr.io/untypequicode/looty-* (en CI : celles construites depuis le commit).
# Il faut Docker Compose, Node ≥ 22, et les ports 80 et 443 libres.
set -eu

repo=$(cd "$(dirname "$0")/../.." && pwd)
dir=$(mktemp -d)
project=looty-caddy-test

compose() {
	docker compose -p "$project" -f "$dir/docker-compose.yml" "$@"
}

cleanup() {
	status=$?
	if [ "$status" -ne 0 ]; then
		compose logs --tail 80 || true
	fi
	compose down -v --remove-orphans || true
	# Les données (PostgreSQL, Caddy, fichiers envoyés) appartiennent à root : les supprimer depuis un conteneur.
	docker run --rm -v "$dir":/d alpine rm -rf /d/data /d/backend || true
	rm -rf "$dir"
	exit "$status"
}
trap cleanup EXIT INT TERM

sh "$repo/deploy/render.sh" caddy "$dir"
cat > "$dir/.env" <<EOF
APP_NAME=looty
APP_DOMAIN=localhost
BASE_URL=https://localhost
LOOTY_VERSION=${LOOTY_VERSION:-latest}
DB_NAME=looty_db
DB_USER=looty
DB_PASSWORD=$(od -An -N16 -tx1 /dev/urandom | tr -d ' \n')
LOOTY_SECRET_KEY=$(od -An -N32 -tx1 /dev/urandom | tr -d ' \n')
OIDC_ISSUER=
OIDC_CLIENT_ID=
OIDC_CLIENT_SECRET=
EOF

compose up -d

# Caddy crée la racine de son AC interne au premier certificat pour « localhost ».
i=0
until compose exec -T caddy test -f /data/caddy/pki/authorities/local/root.crt; do
	i=$((i + 1))
	[ "$i" -lt 60 ] || { echo "Caddy did not create its internal CA" >&2; exit 1; }
	sleep 1
done
compose exec -T caddy cat /data/caddy/pki/authorities/local/root.crt > "$dir/root.crt"

NODE_EXTRA_CA_CERTS="$dir/root.crt" node "$repo/deploy/test/smoke.mjs" https://localhost
