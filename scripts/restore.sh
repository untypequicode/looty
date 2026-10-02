#!/bin/bash

if [ -z "$1" ]; then
    echo "Usage: $0 <backup_file.sql.gz>"
    echo "Example: $0 ./backups/backup_2023-10-25_12-00-00.sql.gz"
    exit 1
fi

BACKUP_FILE=$1

if [ ! -f "$BACKUP_FILE" ]; then
    echo "Error: File $BACKUP_FILE not found!"
    exit 1
fi

DB_CONTAINER=$(docker compose -f docker-compose.prod.yml ps -q postgres 2>/dev/null || docker compose ps -q postgres)
DB_USER=${DB_USER:-postgres}
DB_NAME=${DB_NAME:-looty_db}

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

# Drop and recreate the database to ensure a clean restore
docker exec "$DB_CONTAINER" psql -U "$DB_USER" -d postgres -c "DROP DATABASE IF EXISTS $DB_NAME WITH (FORCE);"
docker exec "$DB_CONTAINER" psql -U "$DB_USER" -d postgres -c "CREATE DATABASE $DB_NAME;"

# Restore
gunzip -c "$BACKUP_FILE" | docker exec -i "$DB_CONTAINER" psql -U "$DB_USER" -d "$DB_NAME"

if [ $? -eq 0 ]; then
    echo "Database restore completed successfully!"
    
    # Try to find corresponding uploads backup
    UPLOADS_FILE="${BACKUP_FILE/backup_/uploads_}"
    UPLOADS_FILE="${UPLOADS_FILE/.sql/.tar}"
    if [ -f "$UPLOADS_FILE" ]; then
        echo "Found corresponding uploads backup. Restoring..."
        tar -xzf "$UPLOADS_FILE"
        echo "Uploads restored!"
    fi
else
    echo "Error: Restore failed!"
    exit 1
fi
