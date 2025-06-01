use anyhow::Result;
use oauth2::{basic::BasicClient, RefreshToken, TokenResponse};
use tracing::{error, info, warn};

use crate::database::models::OAuthToken;

#[derive(Debug)]
pub enum OAuthError {
    NoRefreshToken,
    TokenExpired,
    RefreshFailed(String),
    InvalidToken,
}

impl std::fmt::Display for OAuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OAuthError::NoRefreshToken => write!(f, "No refresh token available"),
            OAuthError::TokenExpired => write!(f, "Token has expired"),
            OAuthError::RefreshFailed(msg) => write!(f, "Token refresh failed: {}", msg),
            OAuthError::InvalidToken => write!(f, "Invalid token format"),
        }
    }
}

impl std::error::Error for OAuthError {}

pub async fn refresh_token_if_needed(
    client: &BasicClient,
    token: &OAuthToken,
) -> Result<Option<OAuthToken>, OAuthError> {
    if !token.expires_soon() {
        return Ok(None);
    }

    info!(
        "Token expires soon, attempting refresh for user {}",
        token.user_id
    );

    let refresh_token_str = token
        .refresh_token
        .as_ref()
        .ok_or(OAuthError::NoRefreshToken)?;

    let refresh_token = RefreshToken::new(refresh_token_str.clone());

    match client
        .exchange_refresh_token(&refresh_token)
        .request_async(oauth2::reqwest::async_http_client)
        .await
    {
        Ok(token_result) => {
            info!("Successfully refreshed token for user {}", token.user_id);

            let expires_at = token_result.expires_in().map(|duration| {
                (chrono::Utc::now() + chrono::Duration::from_std(duration).unwrap_or_default())
                    .naive_utc()
            });

            // Create new token with refreshed values
            let new_token = OAuthToken {
                id: token.id,
                user_id: token.user_id,
                access_token: token_result.access_token().secret().clone(),
                refresh_token: token_result
                    .refresh_token()
                    .map(|rt| rt.secret().clone())
                    .or_else(|| token.refresh_token.clone()), // Keep old refresh token if new one not provided
                expires_at,
                scope: token.scope.clone(), // Keep existing scope
                created_at: token.created_at,
                updated_at: Some(chrono::Utc::now().naive_utc()),
            };

            Ok(Some(new_token))
        }
        Err(e) => {
            error!("Failed to refresh token for user {}: {}", token.user_id, e);
            Err(OAuthError::RefreshFailed(e.to_string()))
        }
    }
}

pub fn validate_token_scopes(token: &OAuthToken) -> Result<(), OAuthError> {
    let required_scopes = [
        "https://www.googleapis.com/auth/meetings.space.created",
    ];

    if let Some(ref scope) = token.scope {
        let token_scopes: Vec<&str> = scope.split_whitespace().collect();

        for required_scope in &required_scopes {
            if !token_scopes.contains(required_scope) {
                warn!("Token missing required scope: {}", required_scope);
                return Err(OAuthError::InvalidToken);
            }
        }

        Ok(())
    } else {
        warn!("Token has no scope information");
        Err(OAuthError::InvalidToken)
    }
}

/// Check if a token is valid and not expired
pub fn is_token_valid(token: &OAuthToken) -> bool {
    !token.is_expired() && validate_token_scopes(token).is_ok()
}
