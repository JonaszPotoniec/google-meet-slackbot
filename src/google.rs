use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
struct CreateSpaceRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    config: Option<SpaceConfig>,
}

#[derive(Debug, Serialize)]
struct SpaceConfig {
    #[serde(rename = "accessType")]
    access_type: String,
}

#[derive(Debug, Deserialize)]
struct Space {
    name: String,
    #[serde(rename = "meetingUri")]
    meeting_uri: String,
    #[serde(rename = "meetingCode")]
    meeting_code: String,
}

pub async fn create_meet_space(access_token: &str) -> Result<String> {
    let client = Client::new();

    // Create a minimal space request
    let space_request = CreateSpaceRequest {
        config: Some(SpaceConfig {
            access_type: "OPEN".to_string(), // Anyone with the link can join
        }),
    };

    let response = client
        .post("https://meet.googleapis.com/v2/spaces")
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Content-Type", "application/json")
        .json(&space_request)
        .send()
        .await?;

    if !response.status().is_success() {
        let error_text = response.text().await?;
        return Err(anyhow!("Failed to create Meet space: {}", error_text));
    }

    let space: Space = response.json().await?;
    Ok(space.meeting_uri)
}
