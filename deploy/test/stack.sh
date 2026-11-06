#!/bin/sh
# Démarre la pile produite par deploy/render.sh sur https://localhost, lance deploy/test/smoke.mjs, puis arrête et
# supprime la pile.
#   LOOTY_VERSION=latest sh deploy/test/stack.sh caddy|traefik [pocket-id]
# caddy : le compose rendu tel quel ; certificat de l'AC interne de Caddy.
# traefik : le compose rendu ne contient pas Traefik (il est « déjà là » chez la personne) ; le test en ajoute un,
# sur le réseau de la pile, qui sert les routeurs des étiquettes en HTTPS avec un certificat d'une AC créée ici.
# Avec pocket-id : le fournisseur OIDC inclus sur https://auth.localhost ; vérifie en plus que pocket-id-init crée le
# client (et ne change rien relancé), puis le parcours complet de deploy/test/pocket-id.mjs (premier compte, connexion).
# LOOTY_VERSION choisit les images ghcr.io/untypequicode/looty-* (en CI : celles construites depuis le commit).
# Il faut Docker Compose, Node ≥ 22, openssl (traefik), et les ports 80 et 443 libres.
set -eu

usage() {
	echo "usage: $0 caddy|traefik [pocket-id]" >&2
	exit 2
}

repo=$(cd "$(dirname "$0")/../.." && pwd)
[ $# -ge 1 ] || usage
proxy=$1
option=${2:-}
case $proxy in caddy | traefik) ;; *) usage ;; esac
case $option in '' | pocket-id) ;; *) usage ;; esac
dir=$(mktemp -d)
project=looty-$proxy-test

compose() {
	if [ -f "$dir/test.override.yml" ]; then
		docker compose -p "$project" -f "$dir/docker-compose.yml" -f "$dir/test.override.yml" "$@"
	else
		docker compose -p "$project" -f "$dir/docker-compose.yml" "$@"
	fi
}

cleanup() {
	status=$?
	# Une seule fois (EXIT suit INT et TERM) : un second passage recréerait $dir en root avec docker run.
	trap - EXIT INT TERM
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

random() { od -An -N"$1" -tx1 /dev/urandom | tr -d ' \n'; }

# shellcheck disable=SC2086 # $option : aucun ou un mot
sh "$repo/deploy/render.sh" "$proxy" "$dir" $option
cat > "$dir/.env" <<ENV
APP_NAME=looty
APP_DOMAIN=localhost
BASE_URL=https://localhost
LOOTY_VERSION=${LOOTY_VERSION:-latest}
DB_NAME=looty_db
DB_USER=looty
DB_PASSWORD=$(random 16)
LOOTY_SECRET_KEY=$(random 32)
ENV
if [ -n "$option" ]; then
	provider_id=$(cat /proc/sys/kernel/random/uuid)
	cat >> "$dir/.env" <<ENV
AUTH_DOMAIN=auth.localhost
POCKET_ID_ENCRYPTION_KEY=$(random 32)
POCKET_ID_STATIC_API_KEY=$(random 32)
OIDC_ISSUER=https://auth.localhost
OIDC_CLIENT_ID=looty
OIDC_CLIENT_SECRET=$(random 32)
OIDC_PROVIDER_ID=$provider_id
ENV
else
	cat >> "$dir/.env" <<ENV
OIDC_ISSUER=
OIDC_CLIENT_ID=
OIDC_CLIENT_SECRET=
ENV
fi

if [ "$proxy" = traefik ]; then
	# Une AC propre au test et un certificat pour localhost et auth.localhost, servi par défaut par Traefik.
	mkdir "$dir/certs"
	openssl req -x509 -newkey rsa:2048 -nodes -days 1 -subj /CN=looty-test-ca \
		-keyout "$dir/certs/ca.key" -out "$dir/certs/ca.crt" 2> /dev/null
	openssl req -newkey rsa:2048 -nodes -subj /CN=localhost \
		-keyout "$dir/certs/localhost.key" -out "$dir/certs/localhost.csr" 2> /dev/null
	printf 'subjectAltName=DNS:localhost,DNS:auth.localhost\n' > "$dir/certs/san.ext"
	openssl x509 -req -in "$dir/certs/localhost.csr" -CA "$dir/certs/ca.crt" -CAkey "$dir/certs/ca.key" \
		-CAcreateserial -days 1 -extfile "$dir/certs/san.ext" -out "$dir/certs/localhost.crt" 2> /dev/null
	chmod 644 "$dir/certs/"*
	cp "$dir/certs/ca.crt" "$dir/root.crt"
	cat > "$dir/tls.yml" <<'YAML'
tls:
  stores:
    default:
      defaultCertificate:
        certFile: /certs/localhost.crt
        keyFile: /certs/localhost.key
YAML
	# Le Traefik « déjà en place » : routeurs des étiquettes servis sur l'entrée HTTPS (TLS au niveau de l'entrée,
	# comme le supposent les étiquettes), limité aux conteneurs de cette pile.
	cat > "$dir/test.override.yml" <<YAML
services:
  traefik:
    image: traefik:v3.6
    command:
      - --providers.docker=true
      - --providers.docker.exposedByDefault=false
      - --providers.docker.constraints=Label(\`com.docker.compose.project\`,\`$project\`)
      - --providers.file.filename=/etc/traefik/tls.yml
      - --entrypoints.web.address=:80
      - --entrypoints.web.http.redirections.entrypoint.to=websecure
      - --entrypoints.websecure.address=:443
      - --entrypoints.websecure.http.tls=true
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock:ro
      - ./certs:/certs:ro
      - ./tls.yml:/etc/traefik/tls.yml:ro
YAML
	if [ -n "$option" ]; then
		# Le serveur joint son propre domaine (README) : ici, auth.localhost mène à Traefik sur le réseau Docker.
		cat >> "$dir/test.override.yml" <<'YAML'
    networks:
      default:
        aliases:
          - auth.localhost
  backend:
    environment:
      OIDC_CA_FILE: /certs/ca.crt
    volumes:
      - ./certs:/certs:ro
YAML
	fi
elif [ -n "$option" ]; then
	# Propre au test : le certificat de auth.localhost vient de l'AC interne de Caddy, que Looty doit accepter.
	cat > "$dir/test.override.yml" <<'YAML'
services:
  backend:
    environment:
      OIDC_CA_FILE: /caddy-data/caddy/pki/authorities/local/root.crt
    volumes:
      - ./data/caddy:/caddy-data:ro
YAML
fi

compose up -d

if [ "$proxy" = caddy ]; then
	# Caddy crée la racine de son AC interne au premier certificat pour « localhost ».
	i=0
	until compose exec -T caddy test -f /data/caddy/pki/authorities/local/root.crt; do
		i=$((i + 1))
		[ "$i" -lt 60 ] || { echo "Caddy did not create its internal CA" >&2; exit 1; }
		sleep 1
	done
	compose exec -T caddy cat /data/caddy/pki/authorities/local/root.crt > "$dir/root.crt"
fi

NODE_EXTRA_CA_CERTS="$dir/root.crt" node "$repo/deploy/test/smoke.mjs" https://localhost

if [ -n "$option" ]; then
	# pocket-id-init attend que Pocket ID soit prêt, crée le client, puis s'arrête.
	init=$(compose ps -aq pocket-id-init)
	i=0
	until [ "$(docker inspect -f '{{.State.Status}}' "$init")" = exited ]; do
		i=$((i + 1))
		[ "$i" -lt 120 ] || { echo "pocket-id-init did not finish" >&2; exit 1; }
		sleep 1
	done
	compose logs --no-log-prefix pocket-id-init
	[ "$(docker inspect -f '{{.State.ExitCode}}' "$init")" = 0 ] || { echo "pocket-id-init failed" >&2; exit 1; }
	# Relancé (à chaque « up »), il ne change rien.
	compose run --rm pocket-id-init | tee "$dir/init-again"
	grep -q 'existe déjà' "$dir/init-again"
	grep -q 'déjà enregistré' "$dir/init-again"
	NODE_EXTRA_CA_CERTS="$dir/root.crt" node "$repo/deploy/test/pocket-id.mjs" https://localhost https://auth.localhost "$provider_id"

	# Fournisseur injoignable : Looty refuse la connexion avec le code provider_error (message clair à l'écran)
	# et dit pourquoi dans ses journaux. Redémarré pour vider sa copie de la configuration du fournisseur.
	compose stop pocket-id
	compose restart backend
	i=0
	until curl -sf --cacert "$dir/root.crt" -o /dev/null https://localhost/api/auth/providers; do
		i=$((i + 1))
		[ "$i" -lt 60 ] || { echo "backend did not come back" >&2; exit 1; }
		sleep 1
	done
	curl -s --cacert "$dir/root.crt" -o "$dir/login" -w '%{http_code}' "https://localhost/api/auth/login/$provider_id" > "$dir/login-status"
	if [ "$(cat "$dir/login-status")" != 400 ] || ! grep -q '"error":"provider_error' "$dir/login"; then
		echo "unreachable provider: expected 400 provider_error, got $(cat "$dir/login-status") $(cat "$dir/login")" >&2
		exit 1
	fi
	compose logs backend | grep -q 'découverte impossible sur https://auth.localhost' ||
		{ echo "unreachable provider: no explanation in the backend logs" >&2; exit 1; }
	echo "ok   unreachable provider: clear error (provider_error) and explanation in the logs"
fi
