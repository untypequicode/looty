#!/bin/sh
# Un vrai Traefik devant deux piles Looty rendues par deploy/render.sh (option traefik avec entrypoint, resolver et
# network), toutes sur le réseau Docker externe « traefik-public » :
#   LOOTY_VERSION=ci sh deploy/test/traefik-public.sh
# - Traefik (fournisseur Docker) et un serveur ACME local, Pebble, comme résolveur de certificats « le » ;
# - deux piles, APP_NAME alpha et beta, sur alpha.localhost et beta.localhost (curl --resolve), avec db-backup pour alpha.
# Vérifie : routeurs et services de l'API de Traefik (réseau traefik-public), certificat émis par Pebble, interface et
# API par Traefik, montée en WebSocket (101) sur /collab, noms uniques par APP_NAME (aucune résolution d'une pile à
# l'autre), et une seule sauvegarde de db-backup au démarrage.
# Il faut Docker Compose, curl, openssl, jq, et les ports 80, 443 et 8080 (local) libres. LOOTY_VERSION comme stack.sh.
set -eu

repo=$(cd "$(dirname "$0")/../.." && pwd)
dir=$(mktemp -d)
net=traefik-public
pebble_image=ghcr.io/letsencrypt/pebble:latest
api=http://127.0.0.1:8080

stack() { name=$1; shift; docker compose -p "looty-$name" -f "$dir/$name/docker-compose.yml" "$@"; }
proxy() { docker compose -p looty-traefik-proxy -f "$dir/proxy/docker-compose.yml" "$@"; }

cleanup() {
	status=$?
	trap - EXIT INT TERM
	if [ "$status" -ne 0 ]; then
		stack alpha logs --tail 60 || true
		stack beta logs --tail 30 || true
		proxy logs --tail 60 || true
	fi
	stack alpha down -v --remove-orphans || true
	stack beta down -v --remove-orphans || true
	proxy down -v --remove-orphans || true
	docker network rm "$net" > /dev/null 2>&1 || true
	docker run --rm -v "$dir":/d alpine rm -rf /d/alpha/data /d/alpha/backend /d/beta/data /d/beta/backend || true
	rm -rf "$dir"
	exit "$status"
}
trap cleanup EXIT INT TERM

fail() { echo "FAIL $*" >&2; exit 1; }
ok() { echo "ok   $*"; }
random() { od -An -N"$1" -tx1 /dev/urandom | tr -d ' \n'; }
# retry <secondes> <commande…> : jusqu'à ce que la commande réussisse.
retry() {
	secs=$1; shift
	i=0
	until "$@"; do
		i=$((i + 2))
		[ "$i" -lt "$secs" ] || return 1
		sleep 2
	done
}

docker network create "$net" > /dev/null

# Certificat racine de test de Pebble, pour que Traefik accepte son API ACME.
mkdir -p "$dir/proxy"
id=$(docker create "$pebble_image")
docker cp "$id:/test/certs/pebble.minica.pem" "$dir/proxy/minica.pem"
docker rm "$id" > /dev/null

cat > "$dir/proxy/docker-compose.yml" <<YAML
services:
  pebble:
    image: $pebble_image
    command: ["-config", "/test/config/pebble-config.json"]
    environment:
      # Aucune validation de défi : tout nom est accepté (le test ne prouve pas Let's Encrypt, mais le câblage du résolveur).
      PEBBLE_VA_ALWAYS_VALID: "1"
      PEBBLE_VA_NOSLEEP: "1"
    networks:
      $net:
        aliases: [pebble]
  traefik:
    image: traefik:v3.6
    command:
      - --api.insecure=true
      - --providers.docker=true
      - --providers.docker.exposedByDefault=false
      - --entrypoints.web.address=:80
      - --entrypoints.websecure.address=:443
      - --certificatesresolvers.le.acme.caserver=https://pebble:14000/dir
      - --certificatesresolvers.le.acme.email=test@example.com
      - --certificatesresolvers.le.acme.storage=/acme/acme.json
      - --certificatesresolvers.le.acme.httpchallenge.entrypoint=web
    environment:
      LEGO_CA_CERTIFICATES: /minica.pem
    ports:
      - "80:80"
      - "443:443"
      - "127.0.0.1:8080:8080"
    volumes:
      - /var/run/docker.sock:/var/run/docker.sock:ro
      - ./minica.pem:/minica.pem:ro
      - acme:/acme
    depends_on:
      - pebble
    networks:
      - $net
volumes:
  acme:
networks:
  $net:
    external: true
YAML

for name in alpha beta; do
	mkdir -p "$dir/$name"
	# Seul alpha a la sauvegarde automatique.
	if [ "$name" = alpha ]; then extra=backup; else extra=; fi
	# shellcheck disable=SC2086 # $extra : aucun ou un mot
	sh "$repo/deploy/render.sh" traefik "$dir/$name" entrypoint resolver network $extra
	cat > "$dir/$name/.env" <<ENV
APP_NAME=$name
APP_DOMAIN=$name.localhost
BASE_URL=https://$name.localhost
LOOTY_VERSION=${LOOTY_VERSION:-latest}
DB_NAME=looty_db
DB_USER=looty
DB_PASSWORD=$(random 16)
LOOTY_SECRET_KEY=$(random 32)
OIDC_ISSUER=
OIDC_CLIENT_ID=
OIDC_CLIENT_SECRET=
TRAEFIK_NETWORK=$net
TRAEFIK_ENTRYPOINT=websecure
TRAEFIK_CERT_RESOLVER=le
ENV
done

proxy up -d
stack alpha up -d
stack beta up -d

curl_host() { name=$1; shift; curl -ksS --resolve "$name.localhost:443:127.0.0.1" "$@"; }

for name in alpha beta; do
	# 1. Routeurs et services de l'API de Traefik.
	routers_ready() {
		curl -sf "$api/api/http/routers" -o "$dir/routers.json" || return 1
		[ "$(jq "[.[] | select(.name | startswith(\"$name-\")) | select(.status == \"enabled\")] | length" "$dir/routers.json")" = 3 ]
	}
	retry 60 routers_ready ||
		fail "$name: expected 3 enabled routers in the Traefik API: $(cat "$dir/routers.json")"
	for r in api collab web; do
		jq -e ".[] | select(.name == \"$name-$r@docker\" and .tls.certResolver == \"le\" and (.entryPoints == [\"websecure\"]))" "$dir/routers.json" > /dev/null ||
			fail "$name-$r: router missing, or not on websecure with the le resolver"
	done
	# Les services pointent vers l'adresse du conteneur sur traefik-public (étiquette traefik.docker.network).
	curl -sf "$api/api/http/services" -o "$dir/services.json"
	for pair in api:backend web:frontend collab:collaboration; do
		r=${pair%%:*}
		svc=${pair##*:}
		cid=$(stack "$name" ps -q "$svc")
		ip=$(docker inspect -f "{{(index .NetworkSettings.Networks \"$net\").IPAddress}}" "$cid")
		url=$(jq -r ".[] | select(.name == \"$name-$r@docker\") | .loadBalancer.servers[0].url" "$dir/services.json")
		case $url in http://"$ip":*) ;; *) fail "$name-$r: service url $url, expected $ip on $net" ;; esac
	done
	ok "$name: routers api, collab and web on websecure/le, services on $net"

	# 2. Interface et API par Traefik (le temps que le backend démarre).
	retry 180 curl_host "$name" -f -o /dev/null "https://$name.localhost/api/auth/providers" ||
		fail "$name: API not reachable through Traefik"
	[ "$(curl_host "$name" -o /dev/null -w '%{http_code}' "https://$name.localhost/api/auth/providers")" = 200 ]
	curl_host "$name" -f "https://$name.localhost/" | grep -qi '<html' || fail "$name: no web interface through Traefik"
	ok "$name: interface and API answer through Traefik"

	# 3. Certificat émis par Pebble (résolveur le).
	issuer_ok() {
		echo | openssl s_client -connect 127.0.0.1:443 -servername "$name.localhost" 2> /dev/null |
			openssl x509 -noout -issuer 2> /dev/null | grep -qi pebble
	}
	retry 90 issuer_ok || fail "$name: certificate not issued by Pebble"
	ok "$name: certificate issued by the Pebble ACME server"

	# 4. /collab : la collaboration répond, et la montée en WebSocket passe (101).
	retry 120 sh -c "curl -ksS --resolve $name.localhost:443:127.0.0.1 https://$name.localhost/collab | grep -q Hocuspocus" ||
		fail "$name: /collab does not reach the collaboration service"
	# curl reste connecté après le 101 : on coupe au délai et on lit les en-têtes.
	curl_host "$name" --http1.1 -m 4 -o /dev/null -D "$dir/ws-$name" \
		-H 'Connection: Upgrade' -H 'Upgrade: websocket' -H 'Sec-WebSocket-Version: 13' \
		-H 'Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==' "https://$name.localhost/collab" 2> /dev/null || true
	head -n 1 "$dir/ws-$name" | grep -q ' 101' || fail "$name: WebSocket upgrade on /collab: $(head -n 1 "$dir/ws-$name")"
	ok "$name: WebSocket upgrade on /collab answers 101"
done

# 5. Noms uniques par APP_NAME sur le réseau partagé : le collab d'alpha ne résout que le backend d'alpha.
alpha_backend=$(stack alpha ps -q backend)
beta_backend=$(stack beta ps -q backend)
for pair in alpha:beta beta:alpha; do
	me=${pair%%:*}
	other=${pair##*:}
	stack "$me" exec -T collaboration getent hosts "$me-backend" > "$dir/own" || fail "$me: $me-backend does not resolve"
	if stack "$me" exec -T collaboration getent hosts "$other-backend" > /dev/null; then
		fail "$me: $other-backend resolves from $me (aliases are not unique)"
	fi
	if stack "$me" exec -T collaboration getent hosts "$other-collaboration" > /dev/null; then
		fail "$me: $other-collaboration resolves from $me"
	fi
	case $me in alpha) own_id=$alpha_backend ;; *) own_id=$beta_backend ;; esac
	own_ip=$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}} {{end}}' "$own_id")
	case " $own_ip" in *" $(cut -d' ' -f1 "$dir/own") "*) ;; *) fail "$me: $me-backend resolves to $(cut -d' ' -f1 "$dir/own"), not to its own backend ($own_ip)" ;; esac
done
ok "aliases unique per APP_NAME: alpha and beta never resolve each other's backend or collaboration"

# 6. db-backup (alpha) : un export au démarrage, lisible, et pas de second.
retry 60 sh -c "ls '$dir/alpha/data/db-backups'/looty-*.sql.gz > /dev/null 2>&1" || fail "db-backup wrote no dump"
[ "$(find "$dir/alpha/data/db-backups" -name "looty-*.sql.gz" | wc -l)" = 1 ] || fail "db-backup: expected exactly one dump"
docker run --rm -v "$dir/alpha/data/db-backups":/b alpine sh -c 'gunzip -c /b/looty-*.sql.gz | grep -q "PostgreSQL database dump"' ||
	fail "db-backup: dump is not a readable pg_dump"
stack alpha logs db-backup | grep -q 'pg_dump failed' && fail "db-backup reported a failure"
[ "$(stack alpha ps -q db-backup | wc -l)" = 1 ] || fail "db-backup is not running"
! stack beta config --services | grep -qx db-backup || fail "beta has a db-backup it should not have"
ok "db-backup: one readable dump at startup"
