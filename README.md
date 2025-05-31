# Slack Meet Bot

A Slack bot that generates Google Meet links using the `/meet` command. The bot integrates with Google Calendar API to create calendar events with Meet links and stores meeting data in a SQLite database.

## Prerequisites

- Rust (latest stable version)
- A Slack workspace with admin privileges
- A Google Cloud Platform project with Calendar API enabled

## Setup

### 1. Clone and Build

```bash
git clone <your-repo-url>
cd meet-slack-bot
cargo build
```

### 2. Database Setup

Run the database migrations:

```bash
cargo install sqlx-cli
sqlx migrate run
```

### 3. Google Cloud Configuration

1. Go to the [Google Cloud Console](https://console.cloud.google.com/)
2. Create a new project or select an existing one
3. Enable the Google Calendar API
4. Go to "Credentials" and create OAuth 2.0 Client IDs:
   - Application type: Web application
   - Authorized redirect URIs: `http://localhost:3000/auth/google/callback` (adjust for production)
5. Note down the Client ID and Client Secret

### 4. Slack App Configuration

1. Go to [Slack API](https://api.slack.com/apps) and create a new app
2. Go to "Slash Commands" and create a new command:
   - Command: `/meet`
   - Request URL: `http://your-domain.com/slack/commands` (use ngrok for local testing)
   - Short Description: "Create a Google Meet link"
3. Go to "Basic Information" and note down the "Signing Secret"
4. Install the app to your workspace

### 5. Environment Configuration

Copy the example environment file and fill in your credentials:

```bash
cp .env.example .env
```

Edit `.env` with your actual values:

```bash
# Slack Configuration
SLACK_SIGNING_SECRET=your_actual_signing_secret

# Google OAuth2
GOOGLE_CLIENT_ID=your_client_id.apps.googleusercontent.com
GOOGLE_CLIENT_SECRET=your_client_secret
GOOGLE_REDIRECT_URI=http://localhost:3000/auth/google/callback

# Database
DATABASE_URL=sqlite:app.db

# Server Configuration
PORT=3000

# Logging
RUST_LOG=info
```

## Running the Bot

### Development

For local development, use ngrok to expose your local server:

```bash
# Terminal 1: Start the bot
cargo run

# Terminal 2: Expose with ngrok
ngrok http 3000
```

Update your Slack app's request URL to use the ngrok URL.

### Production

```bash
cargo build --release
./target/release/meet-slack-bot
```

## Usage

1. **First Time Setup**: When you first use `/meet` in Slack, you'll be prompted to authenticate with Google
2. **Creating Meet Links**: After authentication, simply type `/meet` or `/meet Meeting Title` to create a Google Meet link

### Commands

- `/meet` - Creates a Google Meet link with a default title
- `/meet [title]` - Creates a Google Meet link with a custom title

## API Endpoints

- `GET /health` - Health check endpoint
- `POST /slack/commands` - Slack slash command handler
- `GET /auth/google` - Initiate Google OAuth flow
- `GET /auth/google/callback` - Google OAuth callback

## Database Schema

The bot uses SQLite with three main tables:

- **users**: Stores Slack user information
- **oauth_tokens**: Stores Google OAuth tokens for each user
- **meetings**: Stores created meeting information

## Security Features

- **Request Verification**: All Slack requests are verified using HMAC-SHA256 signatures
- **Timestamp Validation**: Protects against replay attacks
- **Token Refresh**: Automatically handles OAuth token refresh
- **Secure Storage**: Sensitive data is properly encrypted and stored

## Development

### Running Tests

```bash
cargo test
```

### Database Migrations

To create a new migration:

```bash
sqlx migrate add <migration_name>
```

To run migrations:

```bash
sqlx migrate run
```

## Troubleshooting

### Common Issues

1. **"Authentication Required" message**: Make sure your Google OAuth credentials are correct and the redirect URI matches
2. **"Signature verification failed"**: Check your Slack signing secret
3. **Database errors**: Ensure the database file exists and migrations have been run
4. **Google API errors**: Verify that the Calendar API is enabled in your Google Cloud project

### Logs

Enable debug logging for more detailed information:

```bash
RUST_LOG=debug cargo run
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Submit a pull request

## Architecture

The bot is structured as follows:

- `src/main.rs` - Application entry point and routing
- `src/handlers/` - HTTP request handlers for Slack and OAuth
- `src/database/` - Database models and operations
- `src/google.rs` - Google Calendar API integration
- `src/auth/` - OAuth flow implementation
- `src/utils/` - Utility functions including Slack verification
- `migrations/` - Database schema migrations

The application uses:

- **Axum** for the web server
- **SQLx** for database operations
- **oauth2** for Google OAuth2 flow
- **reqwest** for HTTP requests
- **tracing** for structured logging
