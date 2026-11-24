#!/bin/sh
# Installe Looty (Docker Compose) : génère les secrets sur cette machine, écrit les fichiers, propose de démarrer.
#   curl -fsSL https://looty.untypequicode.fr/install.sh | sh -s -- --domain looty.exemple.fr --proxy caddy --name "Mon asso" --auth oidc
# Aussi servi par GitHub : https://raw.githubusercontent.com/untypequicode/looty/main/deploy/install.sh
# Voir docs/adr/0005-install-generator-runs-in-the-browser.md : la commande ne porte aucun secret ; les secrets sont
# tirés ici par « openssl rand » ou demandés dans le terminal, jamais passés en argument.
# Les fichiers sont ceux de deploy/render.sh (mêmes modèles que le zip du générateur du site).
#
# Options (sans elles, le script pose les questions) :
#   --domain D      domaine de l'instance (son DNS doit pointer vers cette machine)
#   --proxy P       caddy (HTTPS inclus) ou traefik (étiquettes, pour un Traefik déjà en place)
#   --auth A        oidc (votre fournisseur), bundled-oidc (Pocket ID inclus) ou local (comptes locaux, déconseillé)
#   --name N        nom de l'instance (e-mails)
#   --version X.Y.Z version des images (défaut : latest)
#   --dir D         dossier d'installation (défaut : /opt/looty)
#   --force         écrase une installation existante (le .env est sauvegardé en .env.bak)
#   --upgrade       met à jour les fichiers (modèles) d'une installation en gardant .env
#   --start | --no-start   démarre (docker compose up -d) ou non, sans demander
#   --yes           non interactif : ne pose aucune question (les valeurs viennent des options et de l'environnement)
#   --skip-checks   ne vérifie ni Docker, ni les ports, ni le DNS
# Secrets et réglages d'un fournisseur OIDC, en mode non interactif seulement : variables d'environnement
# OIDC_ISSUER, OIDC_CLIENT_ID, OIDC_CLIENT_SECRET (pas d'option : un argument se retrouve dans l'historique du shell).
# Pour les tests : LOOTY_TEMPLATES=<dossier deploy/> utilise un dossier local au lieu de télécharger les modèles ;
# LOOTY_REF=<ref git> choisit les modèles téléchargés (défaut : web-vVERSION, ou main).
set -eu

repo=untypequicode/looty
domain=
proxy=
auth=
name=
version=
dir=/opt/looty
force=0
upgrade=0
start=ask
yes=0
checks=1
provider_id=
oidc_issuer_=
oidc_client_=
smtp_host_=
smtp_port_=
smtp_user_=
smtp_from_=

die() {
	echo "install.sh: $*" >&2
	exit 1
}
note() { echo "==> $*"; }

while [ $# -gt 0 ]; do
	case $1 in
	--domain | --proxy | --auth | --name | --version | --dir)
		[ $# -ge 2 ] || die "$1 needs a value"
		case $1 in
		--domain) domain=$2 ;;
		--proxy) proxy=$2 ;;
		--auth) auth=$2 ;;
		--name) name=$2 ;;
		--version) version=$2 ;;
		--dir) dir=$2 ;;
		esac
		shift 2
		;;
	--force) force=1; shift ;;
	--upgrade) upgrade=1; shift ;;
	--start) start=yes; shift ;;
	--no-start) start=no; shift ;;
	--yes | -y | --non-interactive) yes=1; shift ;;
	--skip-checks) checks=0; shift ;;
	-h | --help)
		sed -n '2,/^set -eu/p' "$0" | sed '$d' | sed 's/^# \{0,1\}//'
		exit 0
		;;
	*) die "unknown option $1 (see --help)" ;;
	esac
done

# Questions : seulement si on a un terminal (le script lu depuis « curl | sh » prend ses réponses sur /dev/tty).
interactive=0
if [ "$yes" = 0 ] && [ -r /dev/tty ] && [ -w /dev/tty ]; then interactive=1; fi

# ask VAR "question" [défaut] : remplit VAR s'il est vide.
ask() {
	eval "current=\${$1}"
	[ -z "$current" ] || return 0
	if [ "$interactive" = 1 ]; then
		if [ -n "${3:-}" ]; then printf '%s [%s]: ' "$2" "$3" > /dev/tty; else printf '%s: ' "$2" > /dev/tty; fi
		read -r answer < /dev/tty || answer=
		[ -n "$answer" ] || answer=${3:-}
	else
		answer=${3:-}
	fi
	eval "$1=\$answer"
}

# ask_secret VAR "question" : sans écho ; en mode non interactif, VAR doit déjà venir de l'environnement.
ask_secret() {
	eval "current=\${$1:-}"
	[ -z "$current" ] || return 0
	[ "$interactive" = 1 ] || die "$1 is required (set it in the environment; secrets are never options)"
	printf '%s: ' "$2" > /dev/tty
	stty -echo < /dev/tty 2> /dev/null || true
	read -r answer < /dev/tty || answer=
	stty echo < /dev/tty 2> /dev/null || true
	printf '\n' > /dev/tty
	eval "$1=\$answer"
}

random() { openssl rand -hex "$1"; }

uuid() {
	if [ -r /proc/sys/kernel/random/uuid ]; then cat /proc/sys/kernel/random/uuid; else echo "$(random 4)-$(random 2)-4$(random 2 | cut -c2-)-a$(random 2 | cut -c2-)-$(random 6)"; fi
}

env_get() { sed -n "s/^$1=//p" "$dir/.env" | head -n 1; }

# --- Mise à jour -----------------------------------------------------------------------------------------------
if [ "$upgrade" = 1 ]; then
	if [ ! -f "$dir/.env" ] || [ ! -f "$dir/docker-compose.yml" ]; then die "$dir is not a Looty installation (no .env or docker-compose.yml)"; fi
	if grep -q 'traefik\.enable' "$dir/docker-compose.yml"; then proxy=traefik; else proxy=caddy; fi
	[ -z "$(env_get AUTH_DOMAIN)" ] || auth=bundled-oidc
fi

# --- Questions -------------------------------------------------------------------------------------------------
if [ "$upgrade" = 0 ]; then
	ask domain "Domain of the instance (DNS pointing to this machine)"
	ask proxy "Reverse proxy: caddy (HTTPS included) or traefik (already running)" caddy
	ask name "Name of the instance" Looty
	ask auth "Sign-in: oidc (your provider), bundled-oidc (Pocket ID included), local (not recommended)" oidc
	[ -n "$domain" ] || die "--domain is required"
fi
case $proxy in caddy | traefik) ;; *) die "--proxy must be caddy or traefik" ;; esac
case $auth in
oidc | bundled-oidc | local | '') ;;
*) die "--auth must be oidc, bundled-oidc or local" ;;
esac
if [ "$upgrade" = 0 ]; then
	case $domain in *[!A-Za-z0-9.-]* | .* | *. | '') die "invalid domain: $domain" ;; esac
	case $version in '' | [0-9]*.[0-9]*.[0-9]*) ;; *) die "--version must look like 1.2.3" ;; esac
	case $name in *[\"\'\\\$\`]*) die "the name must not contain quotes, backslashes, \$ or backticks" ;; esac
fi

# --- Prérequis -------------------------------------------------------------------------------------------------
need() { command -v "$1" > /dev/null 2>&1 || die "$1 is required but not installed"; }
need openssl
need awk
if [ "$checks" = 1 ]; then
	need docker
	docker compose version > /dev/null 2>&1 || die "Docker Compose v2 is required (docker compose)"
	docker info > /dev/null 2>&1 || die "cannot talk to Docker (is it running, and are you allowed to use it?)"
	if [ "$proxy" = caddy ] && [ "$upgrade" = 0 ] && command -v ss > /dev/null 2>&1; then
		for port in 80 443; do
			if ss -ltn 2> /dev/null | awk -v p=":$port" '$4 ~ p"$" { found = 1 } END { exit !found }'; then
				die "port $port is already in use; free it, or use --proxy traefik behind your own proxy"
			fi
		done
	fi
	if [ "$upgrade" = 0 ] && command -v getent > /dev/null 2>&1; then
		for host in "$domain" $([ "$auth" = bundled-oidc ] && echo "auth.$domain"); do
			getent hosts "$host" > /dev/null 2>&1 || echo "warning: $host does not resolve yet; point its DNS record to this machine before HTTPS can work" >&2
		done
	fi
fi

# --- Existant --------------------------------------------------------------------------------------------------
if [ "$upgrade" = 0 ] && { [ -f "$dir/.env" ] || [ -f "$dir/docker-compose.yml" ]; }; then
	[ "$force" = 1 ] || die "$dir already contains an installation; use --upgrade to update it, or --force to overwrite it"
fi

# --- Modèles ---------------------------------------------------------------------------------------------------
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT INT TERM
if [ -n "${LOOTY_TEMPLATES:-}" ]; then
	cp -R "$LOOTY_TEMPLATES"/. "$work/"
else
	ref=${LOOTY_REF:-}
	[ -n "$ref" ] || { if [ -n "$version" ]; then ref=web-v$version; else ref=main; fi; }
	base=https://raw.githubusercontent.com/$repo/$ref/deploy
	need curl
	mkdir -p "$work/templates"
	note "Downloading the templates ($ref)"
	for file in render.sh templates/docker-compose.yml templates/Caddyfile templates/pocket-id-init.sh; do
		curl -fsSL "$base/$file" -o "$work/$file" || die "cannot download $base/$file"
	done
fi
option=
[ "$auth" != bundled-oidc ] || option=pocket-id

# --- Fichiers --------------------------------------------------------------------------------------------------
mkdir -p "$dir"
chmod 750 "$dir"
rendered="$work/out"
# shellcheck disable=SC2086 # $option : aucun ou un mot
sh "$work/render.sh" "$proxy" "$rendered" $option

if [ "$upgrade" = 1 ]; then
	note "Updating the files in $dir (the .env is kept)"
	if [ -n "$version" ]; then
		if grep -q '^#* *LOOTY_VERSION=' "$dir/.env"; then
			sed "s/^#* *LOOTY_VERSION=.*/LOOTY_VERSION=$version/" "$dir/.env" > "$dir/.env.new"
			chmod 600 "$dir/.env.new"
			mv "$dir/.env.new" "$dir/.env"
		else
			printf 'LOOTY_VERSION=%s\n' "$version" >> "$dir/.env"
		fi
	fi
	for file in "$rendered"/*; do cp "$file" "$dir/$(basename "$file")"; done
else
	if [ "$auth" = oidc ] || [ -z "$auth" ]; then
		ask oidc_issuer_ "OpenID Connect issuer URL (https://…)" "${OIDC_ISSUER:-}"
		OIDC_ISSUER=$oidc_issuer_
		ask oidc_client_ "OpenID Connect client id" "${OIDC_CLIENT_ID:-looty}"
		OIDC_CLIENT_ID=$oidc_client_
		[ -n "$OIDC_ISSUER" ] || die "OIDC_ISSUER is required with --auth oidc"
		ask_secret OIDC_CLIENT_SECRET "OpenID Connect client secret (not shown)"
		[ -n "$OIDC_CLIENT_SECRET" ] || die "OIDC_CLIENT_SECRET is required with --auth oidc"
		provider_id=$(uuid)
		note "Redirect URL to register at your provider: https://$domain/api/auth/callback/$provider_id (see the docs)"
	fi
	smtp=
	if [ "$interactive" = 1 ]; then
		printf 'Configure outgoing e-mail (SMTP) now? You can do it later in Administration > E-mail. [y/N]: ' > /dev/tty
		read -r smtp < /dev/tty || smtp=
	fi

	note "Writing the files in $dir"
	if [ -f "$dir/.env" ]; then cp -p "$dir/.env" "$dir/.env.bak"; fi
	for file in "$rendered"/*; do cp "$file" "$dir/$(basename "$file")"; done
	umask 077
	{
		echo "# Written by the Looty installer (install.sh or the generator on the site). Settings: https://looty.untypequicode.fr/docs/reference/environment/"
		echo "APP_NAME=looty"
		echo "APP_DOMAIN=$domain"
		echo "BASE_URL=https://$domain"
		[ -z "$version" ] || echo "LOOTY_VERSION=$version"
		echo "SMTP_FROM_NAME=$name"
		echo "DB_NAME=looty_db"
		echo "DB_USER=looty"
		echo "DB_PASSWORD=$(random 16)"
		echo "LOOTY_SECRET_KEY=$(openssl rand -base64 32)"
		if [ "$auth" = bundled-oidc ]; then
			echo "AUTH_DOMAIN=auth.$domain"
			echo "POCKET_ID_ENCRYPTION_KEY=$(random 32)"
			echo "POCKET_ID_STATIC_API_KEY=$(random 32)"
			echo "OIDC_ISSUER=https://auth.$domain"
			echo "OIDC_CLIENT_ID=looty"
			echo "OIDC_CLIENT_SECRET=$(random 32)"
			echo "OIDC_PROVIDER_ID=$(uuid)"
		elif [ "$auth" = local ]; then
			echo "LOOTY_LOCAL_ACCOUNTS=true"
		else
			echo "OIDC_ISSUER=$OIDC_ISSUER"
			echo "OIDC_CLIENT_ID=$OIDC_CLIENT_ID"
			echo "OIDC_CLIENT_SECRET=$OIDC_CLIENT_SECRET"
			echo "OIDC_PROVIDER_ID=$provider_id"
		fi
	} > "$dir/.env"
	chmod 600 "$dir/.env"
	umask 022
	case $smtp in
	y | Y | yes | YES)
		ask smtp_host_ "SMTP server"
		ask smtp_port_ "SMTP port" 587
		ask smtp_user_ "SMTP username"
		ask_secret SMTP_PASSWORD "SMTP password (not shown)"
		ask smtp_from_ "Sender address (From)"
		{
			echo "SMTP_HOST=$smtp_host_"
			echo "SMTP_PORT=$smtp_port_"
			echo "SMTP_USERNAME=$smtp_user_"
			echo "SMTP_PASSWORD=$SMTP_PASSWORD"
			echo "SMTP_FROM=$smtp_from_"
		} >> "$dir/.env"
		;;
	esac
fi

# --- Démarrage -------------------------------------------------------------------------------------------------
if [ "$start" = ask ]; then
	start=no
	if [ "$interactive" = 1 ] && [ "$checks" = 1 ]; then
		printf 'Start Looty now (docker compose up -d)? [Y/n]: ' > /dev/tty
		read -r answer < /dev/tty || answer=n
		case $answer in '' | y | Y | yes) start=yes ;; esac
	fi
fi
if [ "$start" = yes ]; then
	note "Starting Looty"
	(cd "$dir" && docker compose pull && docker compose up -d)
else
	echo
	echo "Files written in $dir. To start Looty:  cd $dir && docker compose up -d"
fi

echo
if [ "$upgrade" = 1 ]; then
	echo "Updated. Restart with: cd $dir && docker compose pull && docker compose up -d"
else
	echo "Open https://$domain: the first account to sign in becomes the administrator."
	if [ "$auth" = local ]; then
		echo "Local accounts are not recommended. The sign-in page asks for a setup code printed in the backend logs: docker compose logs backend | grep setup"
	fi
	if [ "$auth" = bundled-oidc ]; then
		echo "Create the first Pocket ID account NOW at https://auth.$domain/setup: that page stays open to anyone until it exists."
	fi
	echo "The secrets are in $dir/.env (readable by its owner only). Keep it, with LOOTY_SECRET_KEY, in your backups."
fi
