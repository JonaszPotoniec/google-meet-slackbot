use crate::crypto::TokenCrypto;
use anyhow::Result;
use chrono::NaiveDateTime;
use sqlx::sqlite::SqlitePool;

pub mod models;
pub use models::*;

#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
    crypto: TokenCrypto,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self> {
        if let Some(parent) =
            std::path::Path::new(database_url.trim_start_matches("sqlite:")).parent()
        {
            tokio::fs::create_dir_all(parent).await?;
        }

        let pool = SqlitePool::connect(database_url).await?;
        let crypto = TokenCrypto::new()?;

        Ok(Self { pool, crypto })
    }

    pub async fn migrate(&self) -> Result<()> {
        sqlx::migrate!("./migrations").run(&self.pool).await?;
        Ok(())
    }

    pub async fn create_user(&self, slack_user_id: &str, slack_team_id: &str) -> Result<User> {
        let user = sqlx::query_as!(
            User,
            r#"
            INSERT INTO users (slack_user_id, slack_team_id)
            VALUES (?1, ?2)
            ON CONFLICT(slack_user_id) DO UPDATE SET
                slack_team_id = excluded.slack_team_id,
                updated_at = CURRENT_TIMESTAMP
            RETURNING id, slack_user_id, slack_team_id, created_at as "created_at!: NaiveDateTime", updated_at as "updated_at!: NaiveDateTime"
            "#,
            slack_user_id,
            slack_team_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(user)
    }

    pub async fn get_user_by_slack_id(&self, slack_user_id: &str) -> Result<Option<User>> {
        let user = sqlx::query_as!(
            User,
            r#"SELECT id as "id!", slack_user_id, slack_team_id, created_at as "created_at!: NaiveDateTime", updated_at as "updated_at!: NaiveDateTime" FROM users WHERE slack_user_id = ?1"#,
            slack_user_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    pub async fn store_oauth_token(&self, token: &OAuthToken) -> Result<()> {
        let encrypted_access_token = self.crypto.encrypt(&token.access_token)?;
        let encrypted_refresh_token = match &token.refresh_token {
            Some(refresh) => Some(self.crypto.encrypt(refresh)?),
            None => None,
        };

        sqlx::query!(
            r#"
            INSERT INTO oauth_tokens (user_id, access_token, refresh_token, expires_at, scope)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(user_id) DO UPDATE SET
                access_token = excluded.access_token,
                refresh_token = excluded.refresh_token,
                expires_at = excluded.expires_at,
                scope = excluded.scope,
                updated_at = CURRENT_TIMESTAMP
            "#,
            token.user_id,
            encrypted_access_token,
            encrypted_refresh_token,
            token.expires_at,
            token.scope
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_oauth_token(&self, user_id: i64) -> Result<Option<OAuthToken>> {
        let encrypted_token = sqlx::query!(
            r#"
            SELECT id, user_id, access_token, refresh_token, expires_at as "expires_at: NaiveDateTime", scope, created_at as "created_at: NaiveDateTime", updated_at as "updated_at: NaiveDateTime"
            FROM oauth_tokens 
            WHERE user_id = ?1
            "#,
            user_id
        )
        .fetch_optional(&self.pool)
        .await?;

        if let Some(encrypted) = encrypted_token {
            let access_token = self.crypto.decrypt(&encrypted.access_token)?;
            let refresh_token = match encrypted.refresh_token {
                Some(encrypted_refresh) => Some(self.crypto.decrypt(&encrypted_refresh)?),
                None => None,
            };

            let token = OAuthToken {
                id: encrypted.id,
                user_id: encrypted.user_id,
                access_token,
                refresh_token,
                expires_at: encrypted.expires_at,
                scope: encrypted.scope,
                created_at: encrypted.created_at,
                updated_at: encrypted.updated_at,
            };

            Ok(Some(token))
        } else {
            Ok(None)
        }
    }

    pub async fn delete_oauth_token(&self, user_id: i64) -> Result<()> {
        sqlx::query!("DELETE FROM oauth_tokens WHERE user_id = ?1", user_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn create_meeting(&self, meeting: &Meeting) -> Result<Meeting> {
        let meeting = sqlx::query_as!(
            Meeting,
            r#"
            INSERT INTO meetings (user_id, meet_link, title)
            VALUES (?1, ?2, ?3)
            RETURNING id, user_id, meet_link, title, created_at as "created_at: NaiveDateTime"
            "#,
            meeting.user_id,
            meeting.meet_link,
            meeting.title
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(meeting)
    }

    pub async fn get_user_meetings(&self, user_id: i64, limit: i64) -> Result<Vec<Meeting>> {
        let meetings = sqlx::query_as!(
            Meeting,
            r#"
            SELECT id, user_id, meet_link, title, created_at as "created_at: NaiveDateTime"
            FROM meetings 
            WHERE user_id = ?1 
            ORDER BY created_at DESC 
            LIMIT ?2
            "#,
            user_id,
            limit
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(meetings)
    }
}
