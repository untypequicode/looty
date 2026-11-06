#!/bin/sh
# Écrit les fichiers d'une installation de Looty à partir des modèles de deploy/templates/.
#   sh deploy/render.sh caddy|traefik <dossier> [pocket-id]
#
# Les modèles n'ont aucune valeur propre à une instance (elles sont toutes dans .env) ; ils n'ont que des blocs
# gardés par le proxy choisi et les options :
#   #@if caddy             (espaces permis devant)
#   …lignes gardées seulement avec caddy…
#   #@end
#   #@if caddy pocket-id   (gardées seulement si caddy ET pocket-id sont choisis)
# Options : pocket-id (fournisseur OIDC inclus, préconfiguré pour Looty).
# Pas d'imbrication. Un modèle dont il ne reste rien n'est pas écrit (le Caddyfile, avec Traefik).
# Le générateur du site (#159) aura le même rendu en JavaScript ; les deux doivent donner les mêmes fichiers.
set -eu

usage() {
	echo "usage: $0 caddy|traefik <dossier> [pocket-id]" >&2
	exit 2
}

[ $# -ge 2 ] || usage
proxy=$1
out=$2
shift 2
case $proxy in caddy | traefik) ;; *) usage ;; esac
enabled=$proxy
for option in "$@"; do
	case $option in pocket-id) enabled="$enabled $option" ;; *) usage ;; esac
done

templates=$(dirname "$0")/templates
mkdir -p "$out"

for template in "$templates"/*; do
	[ -f "$template" ] || continue
	name=$(basename "$template")
	tmp="$out/.$name.tmp"
	awk -v enabled="$enabled" -v file="$name" '
		function fail(msg) { printf "%s:%d: %s\n", file, NR, msg > "/dev/stderr"; failed = 1; exit 1 }
		BEGIN { n = split(enabled, list, " "); for (i = 1; i <= n; i++) on[list[i]] = 1 }
		/^[ \t]*#@if([ \t]|$)/ {
			if (open) fail("#@if inside #@if")
			if (NF < 2) fail("#@if without a condition")
			open = 1
			keep = 1
			for (i = 2; i <= NF; i++) {
				if ($i != "caddy" && $i != "traefik" && $i != "pocket-id") fail("unknown condition " $i)
				if (!($i in on)) keep = 0
			}
			next
		}
		/^[ \t]*#@end[ \t]*$/ {
			if (!open) fail("#@end without #@if")
			open = 0
			next
		}
		/^[ \t]*#@/ { fail("unknown directive") }
		!open || keep { print }
		END { if (!failed && open) { printf "%s: #@if not closed\n", file > "/dev/stderr"; exit 1 } }
	' "$template" > "$tmp" || { rm -f "$tmp"; exit 1; }
	if grep -q '[^[:space:]]' "$tmp"; then mv "$tmp" "$out/$name"; else rm -f "$tmp"; fi
done
