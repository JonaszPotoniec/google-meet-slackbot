use anyhow::{anyhow, Result};
use chrono::{Duration, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
struct CalendarEvent {
    summary: String,
    start: EventDateTime,
    end: EventDateTime,
    #[serde(rename = "conferenceData")]
    conference_data: ConferenceData,
}

#[derive(Debug, Serialize)]
struct EventDateTime {
    #[serde(rename = "dateTime")]
    date_time: String,
    #[serde(rename = "timeZone")]
    time_zone: String,
}

#[derive(Debug, Serialize)]
struct ConferenceData {
    #[serde(rename = "createRequest")]
    create_request: CreateRequest,
}

#[derive(Debug, Serialize)]
struct CreateRequest {
    #[serde(rename = "requestId")]
    request_id: String,
    #[serde(rename = "conferenceSolutionKey")]
    conference_solution_key: ConferenceSolutionKey,
}

#[derive(Debug, Serialize)]
struct ConferenceSolutionKey {
    #[serde(rename = "type")]
    type_: String,
}

#[derive(Debug, Deserialize)]
struct CalendarEventResponse {
    #[serde(rename = "conferenceData")]
    conference_data: Option<ConferenceDataResponse>,
    #[serde(rename = "htmlLink")]
    html_link: String,
}

#[derive(Debug, Deserialize)]
struct ConferenceDataResponse {
    #[serde(rename = "entryPoints")]
    entry_points: Vec<EntryPoint>,
}

#[derive(Debug, Deserialize)]
struct EntryPoint {
    uri: String,
    #[serde(rename = "entryPointType")]
    entry_point_type: String,
}

pub async fn create_calendar_event_with_meet(
    access_token: &str,
    title: Option<String>,
) -> Result<String> {
    let client = Client::new();

    let start_time = Utc::now();
    let end_time = start_time + Duration::hours(1);

    let event_title = title.unwrap_or_else(|| "Meet".to_string());
    let request_id = uuid::Uuid::new_v4().to_string();

    let event = CalendarEvent {
        summary: event_title,
        start: EventDateTime {
            date_time: start_time.to_rfc3339(),
            time_zone: "UTC".to_string(),
        },
        end: EventDateTime {
            date_time: end_time.to_rfc3339(),
            time_zone: "UTC".to_string(),
        },
        conference_data: ConferenceData {
            create_request: CreateRequest {
                request_id,
                conference_solution_key: ConferenceSolutionKey {
                    type_: "hangoutsMeet".to_string(),
                },
            },
        },
    };

    let response = client
        .post("https://www.googleapis.com/calendar/v3/calendars/primary/events")
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Content-Type", "application/json")
        .query(&[("conferenceDataVersion", "1")])
        .json(&event)
        .send()
        .await?;

    if !response.status().is_success() {
        let error_text = response.text().await?;
        return Err(anyhow!("Failed to create calendar event: {}", error_text));
    }

    let event_response: CalendarEventResponse = response.json().await?;

    if let Some(conference_data) = event_response.conference_data {
        for entry_point in conference_data.entry_points {
            if entry_point.entry_point_type == "video" {
                return Ok(entry_point.uri);
            }
        }
    }

    Ok(event_response.html_link)
}
