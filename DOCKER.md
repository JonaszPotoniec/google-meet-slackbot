# Docker Deployment Guide

## 🐳 Docker Compose Setup

This guide explains how to deploy the Slack Meet Bot using Docker Compose with Alpine Linux.

### 📋 Prerequisites

- Docker Engine 20.10+
- Docker Compose 2.0+
- Your Slack and Google OAuth credentials

### 🚀 Quick Start

1. **Clone and navigate to the project:**

   ```bash
   git clone <your-repo>
   cd meet-slack-bot
   ```

2. **Set up environment variables:**

   ```bash
   cp .env.docker .env
   # Edit .env with your actual credentials
   ```

3. **Create data directory:**

   ```bash
   mkdir -p data
   ```

4. **Build and start the services:**

   ```bash
   docker-compose up --build -d
   ```

5. **Check the logs:**
   ```bash
   docker-compose logs -f meet-slack-bot
   ```

### 🔧 Configuration

#### Environment Variables (.env file)

```bash
# Slack Configuration
SLACK_SIGNING_SECRET=your_actual_slack_signing_secret

# Google OAuth2 Configuration
GOOGLE_CLIENT_ID=your_client_id.apps.googleusercontent.com
GOOGLE_CLIENT_SECRET=your_client_secret
GOOGLE_REDIRECT_URI=https://your-domain.com:9001/auth/google/callback

# The redirect URI must match exactly what you configured in Google Cloud Console
```

#### Docker Compose Configuration

The `docker-compose.yml` includes:

- **Health checks**: Automatic container health monitoring
- **Volume mounts**: Persistent SQLite database storage
- **Network isolation**: Custom Docker network for security
- **Restart policy**: Automatic restart unless stopped
- **Port mapping**: Exposes port 9001

### 📁 Directory Structure

```
meet-slack-bot/
├── Dockerfile              # Alpine-based multi-stage build
├── docker-compose.yml      # Service orchestration
├── docker-entrypoint.sh   # Startup script with migrations
├── .dockerignore          # Build optimization
├── .env.docker           # Environment template
└── data/                 # SQLite database persistence
```

### 🔄 Database Migrations

The container automatically runs database migrations on startup using the entrypoint script:

1. Installs `sqlx-cli` (if needed)
2. Runs `sqlx migrate run`
3. Starts the application

### 📊 Monitoring

#### Health Checks

The container includes built-in health checks:

```bash
# Check container health
docker-compose ps

# View health check logs
docker inspect meet-slack-bot --format='{{json .State.Health}}'
```

#### Application Logs

```bash
# Follow logs in real-time
docker-compose logs -f meet-slack-bot

# View last 100 lines
docker-compose logs --tail=100 meet-slack-bot
```

### 🚀 Production Deployment

#### With Reverse Proxy (Recommended)

For production, use a reverse proxy like nginx:

```yaml
# Add to docker-compose.yml
nginx:
  image: nginx:alpine
  ports:
    - "80:80"
    - "443:443"
  volumes:
    - ./nginx.conf:/etc/nginx/nginx.conf
    - ./ssl:/etc/nginx/ssl
  depends_on:
    - meet-slack-bot
```

#### Environment-specific Overrides

Create `docker-compose.prod.yml`:

```yaml
version: "3.8"
services:
  meet-slack-bot:
    environment:
      RUST_LOG: warn
    deploy:
      resources:
        limits:
          memory: 256M
        reservations:
          memory: 128M
```

Deploy with:

```bash
docker-compose -f docker-compose.yml -f docker-compose.prod.yml up -d
```

### 🛠️ Development

#### Local Development with Docker

```bash
# Build development image
docker-compose up --build

# Run with hot reload (requires volume mount for src)
docker-compose -f docker-compose.dev.yml up
```

#### Debugging

```bash
# Access container shell
docker-compose exec meet-slack-bot sh

# Check database
docker-compose exec meet-slack-bot sqlite3 /app/data/app.db ".tables"

# Manual migration
docker-compose exec meet-slack-bot sqlx migrate run
```

### 🔧 Maintenance

#### Backup Database

```bash
# Create backup
docker-compose exec meet-slack-bot sqlite3 /app/data/app.db ".backup '/app/data/backup.db'"

# Copy backup to host
docker cp meet-slack-bot:/app/data/backup.db ./backup-$(date +%Y%m%d).db
```

#### Update Application

```bash
# Pull latest changes
git pull

# Rebuild and restart
docker-compose up --build -d

# Clean up old images
docker image prune -f
```

#### Scale (if needed)

```bash
# Scale to multiple instances (requires load balancer)
docker-compose up --scale meet-slack-bot=3 -d
```

### 🚨 Troubleshooting

#### Common Issues

1. **Port already in use:**

   ```bash
   sudo lsof -ti:9001 | xargs kill -9
   ```

2. **Database locked:**

   ```bash
   docker-compose down
   sudo rm data/app.db-wal data/app.db-shm
   docker-compose up -d
   ```

3. **Migration failed:**

   ```bash
   docker-compose exec meet-slack-bot sqlx migrate info
   docker-compose exec meet-slack-bot sqlx migrate run --dry-run
   ```

4. **Container won't start:**
   ```bash
   docker-compose logs meet-slack-bot
   docker-compose exec meet-slack-bot sh
   ```

#### Performance Monitoring

```bash
# Monitor resource usage
docker stats meet-slack-bot

# Check container processes
docker-compose exec meet-slack-bot top
```

### 📝 Commands Reference

```bash
# Start services
docker-compose up -d

# Stop services
docker-compose down

# View logs
docker-compose logs -f

# Restart service
docker-compose restart meet-slack-bot

# Update and restart
docker-compose pull && docker-compose up -d

# Clean up
docker-compose down -v --remove-orphans
docker system prune -f
```

## ✅ Deployment Checklist

- [ ] Set up environment variables in `.env`
- [ ] Configure Google Cloud OAuth redirect URI
- [ ] Configure Slack app endpoint
- [ ] Create data directory with proper permissions
- [ ] Test health endpoint
- [ ] Verify database migrations
- [ ] Check application logs
- [ ] Test Slack command integration
- [ ] Set up monitoring/alerting
- [ ] Configure backup strategy
