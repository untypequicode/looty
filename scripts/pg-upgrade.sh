#!/usr/bin/env bash
# Passe PostgreSQL 15 -> 18 d'une pile Docker Compose Looty (dump complet puis restauration dans un nouveau volume).
#
#   scripts/pg-upgrade.sh [docker-compose.prod.yml]     (à lancer sur le serveur, dans le dossier du compose)
#
# Étapes : dump (pg_dumpall) -> arrêt de la pile -> compose modifié (image 18 + nouveau volume) -> restauration -> redémarrage.
# L'ancien volume `postgres_data` n'est PAS supprimé : retour arrière = remettre le compose sauvegardé (.pre-pg18) et `up -d`.
# Compter quelques minutes d'indisponibilité (le site est arrêté pendant l'opération).
set -euo pipefail
FILE="${1:-docker-compose.prod.yml}"
DC=(docker compose -f "$FILE")
BACKUPS="${BACKUPS_DIR:-./backend/backups}"
STAMP="$(date +%Y%m%d-%H%M%S)"
DUMP="$BACKUPS/pg15-before-18-$STAMP.sql"

grep -q 'postgres:15' "$FILE" || { echo "$FILE n'utilise pas postgres:15 : rien à faire (ou déjà migré)." >&2; exit 1; }
[ -f .env ] && set -a && . ./.env && set +a
DB_USER="${DB_USER:-postgres}"; DB_NAME="${DB_NAME:-looty_db}"
mkdir -p "$BACKUPS"

echo "1/5 Dump de la base actuelle -> $DUMP"
"${DC[@]}" up -d postgres
until "${DC[@]}" exec -T postgres pg_isready -q -U "$DB_USER" -d "$DB_NAME"; do sleep 1; done
"${DC[@]}" exec -T postgres pg_dumpall -U "$DB_USER" > "$DUMP"
[ "$(wc -c < "$DUMP")" -gt 1000 ] || { echo "Dump suspect (trop petit), arrêt." >&2; exit 1; }
grep -q "CREATE DATABASE $DB_NAME" "$DUMP" || grep -q "\\\\connect $DB_NAME" "$DUMP" || { echo "La base $DB_NAME est absente du dump, arrêt." >&2; exit 1; }

echo "2/5 Arrêt de la pile"
"${DC[@]}" down

echo "3/5 Compose : image 18 + nouveau volume (copie de l'ancien : $FILE.pre-pg18)"
cp "$FILE" "$FILE.pre-pg18"
sed -i \
  -e 's|postgres:15-alpine|postgres:18-alpine|' \
  -e 's|postgres_data:/var/lib/postgresql/data|postgres_data_18:/var/lib/postgresql|' \
  -e 's|^  postgres_data:|  postgres_data_18:|' "$FILE"
grep -q 'postgres_data_18:/var/lib/postgresql' "$FILE" && grep -q '^  postgres_data_18:' "$FILE" \
  || { echo "Le compose n'a pas la forme attendue : voir $FILE.pre-pg18 et modifier à la main (image postgres:18-alpine, volume monté sur /var/lib/postgresql)." >&2; cp "$FILE.pre-pg18" "$FILE"; exit 1; }

echo "4/5 Restauration dans PostgreSQL 18"
"${DC[@]}" up -d postgres
until "${DC[@]}" exec -T postgres pg_isready -q -U "$DB_USER" -d postgres; do sleep 1; done
"${DC[@]}" exec -T postgres psql -v ON_ERROR_STOP=0 -q -U "$DB_USER" -d postgres < "$DUMP" > /dev/null

echo "5/5 Redémarrage de la pile"
"${DC[@]}" up -d
sleep 5
"${DC[@]}" exec -T postgres psql -U "$DB_USER" -d "$DB_NAME" -Atc "select version(); select count(*) || ' utilisateurs' from users;"
echo "Terminé. Dump conservé : $DUMP ; ancien volume conservé (postgres_data) tant que vous ne le supprimez pas."
