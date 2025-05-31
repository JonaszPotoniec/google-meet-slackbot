#!/bin/sh

# Wait for database to be ready and run migrations
echo "Running database migrations..."

# Ensure the data directory exists and has proper permissions
mkdir -p /app/data
chown -R $(id -u):$(id -g) /app/data

# Install sqlx-cli if not present (for migrations)
if ! command -v sqlx >/dev/null 2>&1; then
    echo "Installing sqlx-cli..."
    cargo install sqlx-cli --no-default-features --features sqlite
fi

# Create the database file if it doesn't exist
if [ ! -f "/app/data/app.db" ]; then
    echo "Creating database file..."
    touch /app/data/app.db
    chmod 644 /app/data/app.db
fi

# Run migrations
sqlx migrate run --database-url "${DATABASE_URL}"

if [ $? -eq 0 ]; then
    echo "Migrations completed successfully"
else
    echo "Migration failed, exiting..."
    exit 1
fi

# Start the application
echo "Starting Meet Slack Bot..."
exec ./meet-slack-bot
