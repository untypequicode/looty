#!/usr/bin/env bash
# Sauvegarde de la base (pg_dump) et des fichiers envoyés (backend/uploads) dans ./backups ; garde les 7 dernières.
# À lancer depuis la racine du dépôt (là où se trouvent docker-compose*.yml et .env). L'administration de Looty propose
# aussi des sauvegardes complètes et planifiées (Administration → Sauvegardes).
set -euo pipefail
cd "$(dirname "$0")/.."
# Mêmes identifiants que docker compose (.env), sinon les valeurs par défaut des fichiers compose.
if [ -f .env ]; then set -a; . ./.env; set +a; fi
DB_USER=${DB_USER:-postgres}
DB_NAME=${DB_NAME:-looty_db}
BACKUP_DIR="./backups"
DATE=$(date +%Y-%m-%d_%H-%M-%S)

DB_CONTAINER=$(docker compose -f docker-compose.prod.yml ps -q postgres 2>/dev/null || true)
[ -n "$DB_CONTAINER" ] || DB_CONTAINER=$(docker compose ps -q postgres 2>/dev/null || true)
if [ -z "$DB_CONTAINER" ]; then
    echo "Error: PostgreSQL container is not running."
    exit 1
fi

mkdir -p "$BACKUP_DIR"
BACKUP_FILE="$BACKUP_DIR/backup_${DATE}.sql.gz"
echo "Creating backup to $BACKUP_FILE..."
if ! docker exec "$DB_CONTAINER" pg_dump -U "$DB_USER" -d "$DB_NAME" | gzip > "$BACKUP_FILE"; then
    echo "Error: Backup failed!"
    rm -f "$BACKUP_FILE"
    exit 1
fi
echo "Database backup completed."

if [ -d ./backend/uploads ]; then
    echo "Backing up uploads..."
    tar -czf "$BACKUP_DIR/uploads_${DATE}.tar.gz" ./backend/uploads
fi
echo "All backups done!"

echo "Cleaning up old backups (keeping 7 most recent)..."
find "$BACKUP_DIR" -maxdepth 1 -name 'backup_*.sql.gz' | sort -r | tail -n +8 | xargs -r rm --
find "$BACKUP_DIR" -maxdepth 1 -name 'uploads_*.tar.gz' | sort -r | tail -n +8 | xargs -r rm --
