#!/bin/bash

# Configuration
BACKUP_DIR="./backups"
DATE=$(date +%Y-%m-%d_%H-%M-%S)
DB_CONTAINER=$(docker compose -f docker-compose.prod.yml ps -q postgres 2>/dev/null || docker compose ps -q postgres)
DB_USER=${DB_USER:-postgres}
DB_NAME=${DB_NAME:-looty_db}

if [ -z "$DB_CONTAINER" ]; then
    echo "Error: PostgreSQL container is not running."
    exit 1
fi

# Ensure backup directory exists
mkdir -p "$BACKUP_DIR"

# Perform backup
BACKUP_FILE="$BACKUP_DIR/backup_${DATE}.sql.gz"
echo "Creating backup to $BACKUP_FILE..."

docker exec "$DB_CONTAINER" pg_dump -U "$DB_USER" -d "$DB_NAME" | gzip > "$BACKUP_FILE"

if [ $? -eq 0 ]; then
    echo "Backup completed successfully!"
    echo "Also backing up uploads..."
    tar -czf "$BACKUP_DIR/uploads_${DATE}.tar.gz" ./backend/uploads 2>/dev/null || echo "No uploads folder found or error during tar."
    echo "All backups done!"
    
    # Keep only last 7 backups
    echo "Cleaning up old backups (keeping 7 most recent)..."
    ls -t $BACKUP_DIR/backup_*.sql.gz | tail -n +8 | xargs -r rm --
    ls -t $BACKUP_DIR/uploads_*.tar.gz | tail -n +8 | xargs -r rm --
else
    echo "Error: Backup failed!"
    rm -f "$BACKUP_FILE"
    exit 1
fi
