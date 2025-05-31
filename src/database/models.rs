use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub slack_user_id: String,
    pub slack_team_id: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthToken {
    pub id: Option<i64>,
    pub user_id: i64,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<NaiveDateTime>,
    pub scope: Option<String>,
    pub created_at: Option<NaiveDateTime>,
    pub updated_at: Option<NaiveDateTime>,
}

impl OAuthToken {
    pub fn new(
        user_id: i64,
        access_token: String,
        refresh_token: Option<String>,
        expires_at: Option<DateTime<Utc>>,
        scope: Option<String>,
    ) -> Self {
        Self {
            id: None,
            user_id,
            access_token,
            refresh_token,
            expires_at: expires_at.map(|dt| dt.naive_utc()),
            scope,
            created_at: None,
            updated_at: None,
        }
    }

    pub fn is_expired(&self) -> bool {
        match self.expires_at {
            Some(expires_at) => expires_at <= Utc::now().naive_utc(),
            None => false,
        }
    }

    pub fn expires_soon(&self) -> bool {
        match self.expires_at {
            Some(expires_at) => {
                let five_minutes_from_now = (Utc::now() + chrono::Duration::minutes(5)).naive_utc();
                expires_at <= five_minutes_from_now
            }
            None => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meeting {
    pub id: Option<i64>,
    pub user_id: i64,
    pub meet_link: String,
    pub title: Option<String>,
    pub created_at: Option<NaiveDateTime>,
}

impl Meeting {
    pub fn new(user_id: i64, meet_link: String, title: Option<String>) -> Self {
        Self {
            id: None,
            user_id,
            meet_link,
            title,
            created_at: None,
        }
    }
}
