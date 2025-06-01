use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{error, info, instrument, warn};

use crate::auth::oauth::{is_token_valid, refresh_token_if_needed};
use crate::handlers::auth::create_oauth_client;
use crate::utils::{verify_slack_request, SlackVerificationError};
use crate::validation::InputValidator;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct SlashCommandPayload {
    pub token: String,
    pub team_id: String,
    pub team_domain: String,
    pub channel_id: String,
    pub channel_name: String,
    pub user_id: String,
    pub user_name: String,
    pub command: String,
    pub text: Option<String>,
    pub response_url: String,
    pub trigger_id: String,
}

#[derive(Debug, Serialize)]
pub struct SlackResponse {
    pub response_type: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<SlackAttachment>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocks: Option<Vec<Value>>,
}

#[derive(Debug, Serialize)]
pub struct SlackAttachment {
    pub color: String,
    pub title: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actions: Option<Vec<SlackAction>>,
}

#[derive(Debug, Serialize)]
pub struct SlackAction {
    pub name: String,
    pub text: String,
    #[serde(rename = "type")]
    pub action_type: String,
    pub url: String,
    pub style: String,
}

impl SlackResponse {
    pub fn ephemeral(text: String) -> Self {
        Self {
            response_type: "ephemeral".to_string(),
            text,
            attachments: None,
            blocks: None,
        }
    }

    pub fn in_channel(text: String) -> Self {
        Self {
            response_type: "in_channel".to_string(),
            text,
            attachments: None,
            blocks: None,
        }
    }

    pub fn with_auth_prompt(auth_url: String) -> Self {
        let attachment = SlackAttachment {
            color: "warning".to_string(),
            title: "Authentication Required".to_string(),
            text: "You need to authenticate with Google to create Meet links.".to_string(),
            actions: Some(vec![SlackAction {
                name: "auth".to_string(),
                text: "Authenticate with Google".to_string(),
                action_type: "button".to_string(),
                url: auth_url,
                style: "primary".to_string(),
            }]),
        };

        Self {
            response_type: "ephemeral".to_string(),
            text: "🔐 Authentication needed to create Google Meet links".to_string(),
            attachments: Some(vec![attachment]),
            blocks: None,
        }
    }
}

#[instrument(skip(state, headers, body))]
pub async fn handle_slash_command(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Result<Json<SlackResponse>, StatusCode> {
    info!("Received slash command");

    let signature = headers
        .get("x-slack-signature")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            warn!("Missing or invalid X-Slack-Signature header");
            StatusCode::UNAUTHORIZED
        })?;

    let timestamp = headers
        .get("x-slack-request-timestamp")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            warn!("Missing or invalid X-Slack-Request-Timestamp header");
            StatusCode::UNAUTHORIZED
        })?;

    if let Err(e) = verify_slack_request(&state.slack_signing_secret, signature, timestamp, &body) {
        match e {
            SlackVerificationError::RequestTooOld => {
                warn!("Slack request verification failed: request too old");
                return Err(StatusCode::UNAUTHORIZED);
            }
            SlackVerificationError::SignatureMismatch => {
                warn!("Slack request verification failed: signature mismatch");
                return Err(StatusCode::UNAUTHORIZED);
            }
            _ => {
                error!("Slack request verification failed: {}", e);
                return Err(StatusCode::BAD_REQUEST);
            }
        }
    }

    info!("Slack signature verification successful");

    let payload: SlashCommandPayload = serde_urlencoded::from_str(&body).map_err(|e| {
        error!("Failed to parse form data: {}", e);
        StatusCode::BAD_REQUEST
    })?;

    let validator = InputValidator::new();

    if let Err(e) = validator.validate_slack_command(&payload.command) {
        warn!("Invalid command: {}", e);
        return Ok(Json(SlackResponse::ephemeral(
            "❌ Invalid command".to_string(),
        )));
    }

    if let Err(e) = validator.validate_slack_user_id(&payload.user_id) {
        warn!("Invalid user ID: {}", e);
        return Err(StatusCode::BAD_REQUEST);
    }

    if let Err(e) = validator.validate_slack_team_id(&payload.team_id) {
        warn!("Invalid team ID: {}", e);
        return Err(StatusCode::BAD_REQUEST);
    }

    if let Err(e) = validator.validate_slack_channel_id(&payload.channel_id) {
        warn!("Invalid channel ID: {}", e);
        return Err(StatusCode::BAD_REQUEST);
    }

    if let Err(e) = validator.validate_url(&payload.response_url) {
        warn!("Invalid response URL: {}", e);
        return Err(StatusCode::BAD_REQUEST);
    }

    if let Some(ref text) = payload.text {
        if let Err(e) = validator.validate_text_input(text, "command text") {
            warn!("Invalid command text: {}", e);
            return Ok(Json(SlackResponse::ephemeral(
                "❌ Invalid command text. Please check for special characters.".to_string(),
            )));
        }
    }

    if let Err(e) = state
        .rate_limiter
        .check_user_limit(&payload.user_id, "/slack/commands")
        .await
    {
        warn!("Rate limit exceeded for user {}: {}", payload.user_id, e);
        return Ok(Json(SlackResponse::ephemeral(
            "⏱️ Please slow down! You're sending commands too quickly.".to_string(),
        )));
    }

    if let Err(e) = state
        .rate_limiter
        .check_endpoint_limit("/slack/commands")
        .await
    {
        error!("Global rate limit exceeded: {}", e);
        return Ok(Json(SlackResponse::ephemeral(
            "🚫 Service temporarily unavailable due to high load. Please try again later."
                .to_string(),
        )));
    }

    info!("Parsed command: {}", payload.command);

    match payload.command.as_str() {
        "/meet" => handle_meet_command(state, payload).await,
        _ => {
            error!("Unknown command: {}", payload.command);
            Ok(Json(SlackResponse::ephemeral(
                "Unknown command".to_string(),
            )))
        }
    }
}

#[instrument(skip(state))]
async fn handle_meet_command(
    state: AppState,
    payload: SlashCommandPayload,
) -> Result<Json<SlackResponse>, StatusCode> {
    info!("Handling /meet command for user: {}", payload.user_id);

    let user = match state.db.get_user_by_slack_id(&payload.user_id).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            match state
                .db
                .create_user(&payload.user_id, &payload.team_id)
                .await
            {
                Ok(user) => user,
                Err(e) => {
                    error!("Failed to create user: {}", e);
                    return Ok(Json(SlackResponse::ephemeral(
                        "❌ Sorry, there was an error processing your request.".to_string(),
                    )));
                }
            }
        }
        Err(e) => {
            error!("Database error: {}", e);
            return Ok(Json(SlackResponse::ephemeral(
                "❌ Sorry, there was a database error.".to_string(),
            )));
        }
    };

    match state.db.get_oauth_token(user.id).await {
        Ok(Some(mut token)) => {
            if token.is_expired() || token.expires_soon() {
                info!(
                    "Token expired or expiring soon for user {}, attempting refresh",
                    user.id
                );

                let client = match create_oauth_client(&state) {
                    Ok(client) => client,
                    Err(_) => {
                        error!("Failed to create OAuth client for token refresh");
                        return Ok(Json(SlackResponse::ephemeral(
                            "❌ Authentication system error. Please try again.".to_string(),
                        )));
                    }
                };

                match refresh_token_if_needed(&client, &token).await {
                    Ok(Some(refreshed_token)) => {
                        info!("Successfully refreshed token for user {}", user.id);

                        if let Err(e) = state.db.store_oauth_token(&refreshed_token).await {
                            error!("Failed to store refreshed token: {}", e);
                            return Ok(Json(SlackResponse::ephemeral(
                                "❌ Failed to update authentication. Please re-authenticate."
                                    .to_string(),
                            )));
                        }

                        token = refreshed_token;
                    }
                    Ok(None) => {}
                    Err(e) => {
                        warn!("Failed to refresh token for user {}: {}", user.id, e);
                        let auth_url = format!(
                            "{}/auth/google?user_id={}",
                            state
                                .google_redirect_uri
                                .trim_end_matches("/auth/google/callback"),
                            payload.user_id
                        );

                        return Ok(Json(SlackResponse::with_auth_prompt(auth_url)));
                    }
                }
            }

            if !is_token_valid(&token) {
                warn!(
                    "Token invalid or missing required scopes for user {}",
                    user.id
                );
                let auth_url = format!(
                    "{}/auth/google?user_id={}",
                    state
                        .google_redirect_uri
                        .trim_end_matches("/auth/google/callback"),
                    payload.user_id
                );

                return Ok(Json(SlackResponse::with_auth_prompt(auth_url)));
            }

            match create_meet_link(&state, &token, &payload).await {
                Ok(meet_link) => {
                    let meeting = crate::database::models::Meeting::new(
                        user.id,
                        meet_link.clone(),
                        payload.text.clone(),
                    );

                    if let Err(e) = state.db.create_meeting(&meeting).await {
                        error!("Failed to store meeting: {}", e);
                    }

                    Ok(Json(SlackResponse::in_channel(format!(
                        "🎥 Google Meet created by <@{}>: {}",
                        payload.user_name, meet_link
                    ))))
                }
                Err(e) => {
                    error!("Failed to create Meet link: {}", e);
                    Ok(Json(SlackResponse::ephemeral(
                        "❌ Failed to create Google Meet link. Please try again.".to_string(),
                    )))
                }
            }
        }
        Ok(None) => {
            let auth_url = format!(
                "{}/auth/google?user_id={}",
                state
                    .google_redirect_uri
                    .trim_end_matches("/auth/google/callback"),
                payload.user_id
            );

            Ok(Json(SlackResponse::with_auth_prompt(auth_url)))
        }
        Err(e) => {
            let error_message = e.to_string();
            
            if error_message.contains("Invalid encrypted token format") 
                || error_message.contains("Decryption failed") 
                || error_message.contains("Encrypted token too short") {
                
                warn!("Token decryption failed for user {}: {}. Prompting for re-authentication.", user.id, e);
                
                if let Err(delete_err) = state.db.delete_oauth_token(user.id).await {
                    warn!("Failed to delete invalid token: {}", delete_err);
                }
                
                let auth_url = format!(
                    "{}/auth/google?user_id={}",
                    state
                        .google_redirect_uri
                        .trim_end_matches("/auth/google/callback"),
                    payload.user_id
                );

                Ok(Json(SlackResponse::with_auth_prompt(auth_url)))
            } else {
                error!("Failed to get OAuth token: {}", e);
                Ok(Json(SlackResponse::ephemeral(
                    "❌ Sorry, there was an error checking your authentication.".to_string(),
                )))
            }
        }
    }
}

async fn create_meet_link(
    state: &AppState,
    token: &crate::database::models::OAuthToken,
    payload: &SlashCommandPayload,
) -> anyhow::Result<String> {
    let title = match &payload.text {
        Some(text) if !text.trim().is_empty() => Some(text.trim().to_string()),
        _ => None,
    };

    let meet_link =
        crate::google::create_meet_space(&token.access_token).await?;

    let meeting = crate::database::models::Meeting::new(token.user_id, meet_link.clone(), title);

    state.db.create_meeting(&meeting).await?;

    Ok(meet_link)
}
