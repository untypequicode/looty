#!/bin/sh
# Écrit les fichiers d'une installation de Looty à partir des modèles de deploy/templates/.
#   sh deploy/render.sh caddy|traefik <dossier>
#
# Les modèles n'ont aucune valeur propre à une instance (elles sont toutes dans .env) ; ils n'ont que des blocs
# gardés par le proxy choisi :
#   #@if caddy        (espaces permis devant)
#   …lignes gardées seulement avec caddy…
#   #@end
# Pas d'imbrication. Un modèle dont il ne reste rien n'est pas écrit (le Caddyfile, avec Traefik).
# Le générateur du site (#159) aura le même rendu en JavaScript ; les deux doivent donner les mêmes fichiers.
set -eu

usage() {
	echo "usage: $0 caddy|traefik <dossier>" >&2
	exit 2
}

[ $# -eq 2 ] || usage
proxy=$1
out=$2
case $proxy in caddy | traefik) ;; *) usage ;; esac

templates=$(dirname "$0")/templates
mkdir -p "$out"

for template in "$templates"/*; do
	[ -f "$template" ] || continue
	name=$(basename "$template")
	tmp="$out/.$name.tmp"
	awk -v proxy="$proxy" -v file="$name" '
		function fail(msg) { printf "%s:%d: %s\n", file, NR, msg > "/dev/stderr"; failed = 1; exit 1 }
		/^[ \t]*#@if[ \t]/ {
			if (block != "") fail("#@if inside #@if")
			if ($2 != "caddy" && $2 != "traefik") fail("unknown proxy " $2)
			block = $2
			next
		}
		/^[ \t]*#@end[ \t]*$/ {
			if (block == "") fail("#@end without #@if")
			block = ""
			next
		}
		/^[ \t]*#@/ { fail("unknown directive") }
		block == "" || block == proxy { print }
		END { if (!failed && block != "") { printf "%s: #@if %s not closed\n", file, block > "/dev/stderr"; exit 1 } }
	' "$template" > "$tmp" || { rm -f "$tmp"; exit 1; }
	if grep -q '[^[:space:]]' "$tmp"; then mv "$tmp" "$out/$name"; else rm -f "$tmp"; fi
done
