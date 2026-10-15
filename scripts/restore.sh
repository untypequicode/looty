#!/usr/bin/env bash
# Restaure une sauvegarde de scripts/backup.sh : restore.sh <backups/backup_….sql.gz>
# (et les fichiers envoyés de la même date, uploads_….tar.gz, s'ils existent). À lancer depuis la racine du dépôt.
set -euo pipefail

if [ -z "${1:-}" ]; then
    echo "Usage: $0 <backup_file.sql.gz>"
    echo "Example: $0 ./backups/backup_2023-10-25_12-00-00.sql.gz"
    exit 1
fi
BACKUP_FILE=$(realpath "$1")
if [ ! -f "$BACKUP_FILE" ]; then
    echo "Error: File $1 not found!"
    exit 1
fi

cd "$(dirname "$0")/.."
if [ -f .env ]; then set -a; . ./.env; set +a; fi
DB_USER=${DB_USER:-postgres}
DB_NAME=${DB_NAME:-looty_db}

DB_CONTAINER=$(docker compose -f docker-compose.prod.yml ps -q postgres 2>/dev/null || true)
[ -n "$DB_CONTAINER" ] || DB_CONTAINER=$(docker compose ps -q postgres 2>/dev/null || true)
if [ -z "$DB_CONTAINER" ]; then
    echo "Error: PostgreSQL container is not running."
    exit 1
fi

echo "WARNING: This will drop all existing connections and overwrite the database '$DB_NAME'."
read -p "Are you sure you want to continue? (y/N) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Restore cancelled."
    exit 1
fi

echo "Restoring database from $BACKUP_FILE..."
# Base recréée vide pour une restauration propre.
docker exec "$DB_CONTAINER" psql -v ON_ERROR_STOP=1 -U "$DB_USER" -d postgres -c "DROP DATABASE IF EXISTS \"$DB_NAME\" WITH (FORCE);"
docker exec "$DB_CONTAINER" psql -v ON_ERROR_STOP=1 -U "$DB_USER" -d postgres -c "CREATE DATABASE \"$DB_NAME\";"
if ! gunzip -c "$BACKUP_FILE" | docker exec -i "$DB_CONTAINER" psql -q -v ON_ERROR_STOP=1 -U "$DB_USER" -d "$DB_NAME" > /dev/null; then
    echo "Error: Restore failed!"
    exit 1
fi
echo "Database restore completed successfully!"

# Fichiers envoyés de la même sauvegarde (backup_X.sql.gz → uploads_X.tar.gz).
name=$(basename "$BACKUP_FILE")
name="${name/backup_/uploads_}"
UPLOADS_FILE="$(dirname "$BACKUP_FILE")/${name/.sql/.tar}"
if [ -f "$UPLOADS_FILE" ]; then
    echo "Found corresponding uploads backup. Restoring..."
    tar -xzf "$UPLOADS_FILE"
    echo "Uploads restored!"
fi
