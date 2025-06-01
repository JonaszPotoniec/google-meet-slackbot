use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, Redirect},
};
use oauth2::{
    basic::BasicClient, AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, RedirectUrl,
    Scope, TokenResponse, TokenUrl,
};
use serde::Deserialize;
use tracing::{error, info, instrument, warn};

use crate::{database::models::OAuthToken, validation::InputValidator, AppState};

#[derive(Debug, Deserialize)]
pub struct AuthQuery {
    pub user_id: String,
}

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    pub code: String,
    pub state: String,
}

#[instrument(skip(state))]
pub async fn initiate_google_oauth(
    State(state): State<AppState>,
    Query(query): Query<AuthQuery>,
) -> Result<Redirect, StatusCode> {
    info!("Initiating Google OAuth for user: {}", query.user_id);

    let validator = InputValidator::new();
    if let Err(e) = validator.validate_slack_user_id(&query.user_id) {
        warn!("Invalid user ID in OAuth request: {}", e);
        return Err(StatusCode::BAD_REQUEST);
    }

    if let Err(e) = state
        .rate_limiter
        .check_user_limit(&query.user_id, "/auth/google")
        .await
    {
        warn!(
            "Rate limit exceeded for user {} on OAuth: {}",
            query.user_id, e
        );
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    if let Err(e) = state
        .rate_limiter
        .check_endpoint_limit("/auth/google")
        .await
    {
        error!("Global rate limit exceeded for OAuth: {}", e);
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }

    let client = create_oauth_client(&state)?;

    // Create a cryptographically secure state parameter
    use base64::{engine::general_purpose, Engine as _};
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let random_bytes: [u8; 32] = rng.gen();
    let random_state = general_purpose::URL_SAFE_NO_PAD.encode(&random_bytes);
    let state_param = format!("user:{}:{}", query.user_id, random_state);
    let csrf_token = CsrfToken::new(state_param);

    let (auth_url, _) = client
        .authorize_url(|| csrf_token.clone())
        .add_scope(Scope::new(
            "https://www.googleapis.com/auth/calendar".to_string(),
        ))
        .add_scope(Scope::new(
            "https://www.googleapis.com/auth/calendar.events".to_string(),
        ))
        .url();

    info!("Redirecting to Google OAuth: {}", auth_url);
    Ok(Redirect::temporary(auth_url.as_str()))
}

#[instrument(skip(state))]
pub async fn handle_google_callback(
    State(state): State<AppState>,
    Query(query): Query<CallbackQuery>,
) -> Result<Html<String>, StatusCode> {
    info!("Handling Google OAuth callback");

    // Validate OAuth parameters
    let validator = InputValidator::new();
    if let Err(e) = validator.validate_oauth_code(&query.code) {
        warn!("Invalid OAuth code: {}", e);
        return Ok(Html(create_error_page("Invalid authorization code")));
    }

    if let Err(e) = validator.validate_oauth_state(&query.state) {
        warn!("Invalid OAuth state: {}", e);
        return Ok(Html(create_error_page("Invalid authentication state")));
    }

    // Parse and validate state parameter to extract user_id
    let user_id = if query.state.starts_with("user:") {
        let parts: Vec<&str> = query.state.splitn(3, ':').collect();
        if parts.len() >= 2 {
            parts[1]
        } else {
            error!("Invalid state parameter format: {}", query.state);
            return Ok(Html(create_error_page("Invalid authentication state")));
        }
    } else {
        error!("Invalid state parameter format: {}", query.state);
        return Ok(Html(create_error_page("Invalid authentication state")));
    };

    // Validate extracted user ID
    if let Err(e) = validator.validate_slack_user_id(user_id) {
        warn!("Invalid user ID in OAuth callback: {}", e);
        return Ok(Html(create_error_page("Invalid user ID")));
    }

    // Apply rate limiting
    if let Err(e) = state
        .rate_limiter
        .check_user_limit(user_id, "/auth/google/callback")
        .await
    {
        warn!(
            "Rate limit exceeded for user {} on OAuth callback: {}",
            user_id, e
        );
        return Ok(Html(create_error_page(
            "Too many authentication attempts. Please try again later.",
        )));
    }

    if let Err(e) = state
        .rate_limiter
        .check_endpoint_limit("/auth/google/callback")
        .await
    {
        error!("Global rate limit exceeded for OAuth callback: {}", e);
        return Ok(Html(create_error_page(
            "Service temporarily unavailable. Please try again later.",
        )));
    }

    info!("Processing OAuth callback for user: {}", user_id);

    // Exchange authorization code for access token
    let client = create_oauth_client(&state)?;
    let token_result = client
        .exchange_code(AuthorizationCode::new(query.code))
        .request_async(oauth2::reqwest::async_http_client)
        .await;

    match token_result {
        Ok(token) => {
            info!("Successfully obtained OAuth token for user: {}", user_id);

            // Get or create user in database
            let user = match state.db.get_user_by_slack_id(user_id).await {
                Ok(Some(user)) => user,
                Ok(None) => {
                    error!("User not found in database: {}", user_id);
                    return Ok(Html(create_error_page("User not found")));
                }
                Err(e) => {
                    error!("Database error: {}", e);
                    return Ok(Html(create_error_page("Database error")));
                }
            };

            // Calculate expiration time
            let expires_at = token.expires_in().map(|duration| {
                chrono::Utc::now() + chrono::Duration::seconds(duration.as_secs() as i64)
            });

            // Store OAuth token
            let oauth_token = OAuthToken::new(
                user.id,
                token.access_token().secret().to_string(),
                token.refresh_token().map(|t| t.secret().to_string()),
                expires_at,
                Some("https://www.googleapis.com/auth/calendar https://www.googleapis.com/auth/calendar.events".to_string()),
            );

            match state.db.store_oauth_token(&oauth_token).await {
                Ok(_) => {
                    info!("OAuth token stored successfully for user: {}", user_id);
                    Ok(Html(create_success_page()))
                }
                Err(e) => {
                    error!("Failed to store OAuth token: {}", e);
                    Ok(Html(create_error_page("Failed to store authentication")))
                }
            }
        }
        Err(e) => {
            error!("Failed to exchange OAuth code: {}", e);
            Ok(Html(create_error_page("Authentication failed")))
        }
    }
}

pub fn create_oauth_client(state: &AppState) -> Result<BasicClient, StatusCode> {
    let client = BasicClient::new(
        ClientId::new(state.google_client_id.clone()),
        Some(ClientSecret::new(state.google_client_secret.clone())),
        AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string()).map_err(|e| {
            error!("Invalid auth URL: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?,
        Some(
            TokenUrl::new("https://www.googleapis.com/oauth2/v4/token".to_string()).map_err(
                |e| {
                    error!("Invalid token URL: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                },
            )?,
        ),
    )
    .set_redirect_uri(
        RedirectUrl::new(state.google_redirect_uri.clone()).map_err(|e| {
            error!("Invalid redirect URI: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?,
    );

    Ok(client)
}

fn create_success_page() -> String {
    r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>Authentication Successful</title>
        <style>
            body { font-family: Arial, sans-serif; text-align: center; margin: 50px; }
            .success { color: #28a745; }
            .container { max-width: 500px; margin: 0 auto; }
        </style>
    </head>
    <body>
        <div class="container">
            <h1 class="success">✅ Authentication Successful!</h1>
            <p>You've successfully connected your Google account to the Slack bot.</p>
            <p>You can now close this window and return to Slack to use the <code>/meet</code> command.</p>
        </div>
    </body>
    </html>
    "#.to_string()
}

fn create_error_page(error_message: &str) -> String {
    format!(
        r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>Authentication Error</title>
            <style>
                body {{ font-family: Arial, sans-serif; text-align: center; margin: 50px; }}
                .error {{ color: #dc3545; }}
                .container {{ max-width: 500px; margin: 0 auto; }}
            </style>
        </head>
        <body>
            <div class="container">
                <h1 class="error">❌ Authentication Error</h1>
                <p>{}</p>
                <p>Please try again or contact support if the problem persists.</p>
            </div>
        </body>
        </html>
        "#,
        error_message
    )
}
