#@if pocket-id
#!/bin/sh
# Crée le client OIDC de Looty dans le Pocket ID inclus, avec l'identifiant, le secret et l'URL de redirection de .env.
# Lancé par le service pocket-id-init à chaque « docker compose up » ; si le client existe déjà, rien ne change.
# Produit par deploy/render.sh à partir de deploy/templates/pocket-id-init.sh : modifier le modèle, pas ce fichier.
# Utilise l'API de Pocket ID ≥ 2.14 (routes /secrets) avec la clé STATIC_API_KEY.
set -eu

fail() {
	echo "pocket-id-init: ERREUR : $*" >&2
	echo "pocket-id-init: créez le client à la main (README de Looty, « Bundled Pocket ID », configuration manuelle)." >&2
	exit 1
}

key=${POCKET_ID_STATIC_API_KEY:-}
if [ -z "$key" ]; then
	echo "pocket-id-init: POCKET_ID_STATIC_API_KEY est vide : rien à faire (client déjà créé, ou configuration manuelle)."
	exit 0
fi

client=${OIDC_CLIENT_ID:-}
secret=${OIDC_CLIENT_SECRET:-}
provider=${OIDC_PROVIDER_ID:-}
base=${BASE_URL%/}
case $client in '' | *[!A-Za-z0-9._-]*) fail "OIDC_CLIENT_ID doit être fait de lettres, chiffres, « . », « _ » ou « - » (ex. looty)." ;; esac
case $secret in *[!A-Za-z0-9._~+/=-]*) fail "OIDC_CLIENT_SECRET ne doit contenir que des lettres, des chiffres et . _ ~ + / = -" ;; esac
[ "${#secret}" -ge 16 ] || fail "OIDC_CLIENT_SECRET doit faire au moins 16 caractères (openssl rand -hex 32)."
echo "$provider" | grep -Eq '^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$' ||
	fail "OIDC_PROVIDER_ID doit être un UUID (uuidgen)."
case $base in https://*) ;; *) fail "BASE_URL doit commencer par https:// (Pocket ID refuse les redirections en http)." ;; esac
callback="$base/api/auth/callback/$provider"

body=$(mktemp)
# api MÉTHODE CHEMIN [DONNÉES JSON] : écrit la réponse dans $body, affiche le code HTTP (000 si injoignable : curl l'écrit lui-même).
api() {
	set -- "$1" "$POCKET_ID_URL/api$2" "${3:-}"
	if [ -n "$3" ]; then
		curl -sS -o "$body" -w '%{http_code}' -X "$1" -H "X-API-Key: $key" -H 'Content-Type: application/json' --data "$3" "$2" || true
	else
		curl -sS -o "$body" -w '%{http_code}' -X "$1" -H "X-API-Key: $key" "$2" || true
	fi
}

# 1. Le client, créé s'il n'existe pas.
auto_secret=
code=$(api GET "/oidc/clients/$client")
case $code in
200)
	echo "pocket-id-init: le client « $client » existe déjà."
	grep -qF "\"$callback\"" "$body" ||
		echo "pocket-id-init: attention : son URL de redirection n'est pas $callback (BASE_URL ou OIDC_PROVIDER_ID ont changé ?). À corriger dans Pocket ID, Administration > OIDC Clients." >&2
	;;
404)
	code=$(api POST /oidc/clients "{\"id\":\"$client\",\"name\":\"Looty\",\"callbackURLs\":[\"$callback\"],\"logoutCallbackURLs\":[],\"isPublic\":false,\"pkceEnabled\":true,\"skipConsent\":true,\"launchURL\":\"$base\"}")
	[ "$code" = 201 ] || fail "création du client refusée (HTTP $code) : $(cat "$body")"
	# Pocket ID crée un secret aléatoire avec le client ; on le remplace par celui de .env.
	auto_secret=$(sed -n 's/.*"createdSecret":{"id":"\([^"]*\)".*/\1/p' "$body")
	echo "pocket-id-init: client « $client » créé, redirection vers $callback."
	;;
401 | 403) fail "clé API refusée (HTTP $code) : POCKET_ID_STATIC_API_KEY ne correspond pas à celle de Pocket ID." ;;
000) fail "Pocket ID injoignable à $POCKET_ID_URL." ;;
*) fail "réponse inattendue de Pocket ID (HTTP $code) : $(cat "$body")" ;;
esac

# 2. Le secret de .env, ajouté s'il manque. Pocket ID ne garde que ses 4 premiers caractères en clair.
code=$(api GET "/oidc/clients/$client/secrets")
[ "$code" = 200 ] || fail "lecture des secrets du client refusée (HTTP $code) : $(cat "$body")"
prefix=$(printf %s "$secret" | cut -c1-4)
if grep -qF "\"prefix\":\"$prefix\"" "$body"; then
	echo "pocket-id-init: le secret de OIDC_CLIENT_SECRET est déjà enregistré."
else
	code=$(api POST "/oidc/clients/$client/secrets" "{\"secret\":\"$secret\",\"expiresAt\":null}")
	case $code in 200 | 201) echo "pocket-id-init: secret de OIDC_CLIENT_SECRET enregistré." ;; *) fail "ajout du secret refusé (HTTP $code) : $(cat "$body")" ;; esac
fi

if [ -n "$auto_secret" ]; then
	code=$(api DELETE "/oidc/clients/$client/secrets/$auto_secret")
	case $code in 200 | 204) ;; *) echo "pocket-id-init: attention : le secret créé d'office n'a pas pu être retiré (HTTP $code)." >&2 ;; esac
fi

rm -f "$body"
echo "pocket-id-init: prêt. Créez le premier compte sur https://${AUTH_DOMAIN:-<AUTH_DOMAIN>}/setup, puis connectez-vous à $base."
#@end
