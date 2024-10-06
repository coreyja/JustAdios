use chrono::{DateTime, Utc};
use cja::{
    app_state::AppState as _,
    color_eyre::{self, eyre::Context as _},
    uuid::Uuid,
};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, PgPool};

use crate::AppState;

#[tracing::instrument(err)]
pub async fn setup_db_pool() -> color_eyre::Result<PgPool> {
    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;
    let mut connection = pool.acquire().await?;

    let lock = sqlx::postgres::PgAdvisoryLock::new("just-adios-db-migration-lock");
    let mut lock = lock.acquire(&mut connection).await?;

    sqlx::migrate!().run(lock.as_mut()).await?;

    lock.release_now().await?;
    tracing::info!("Migration lock unlocked");

    Ok(pool)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct DBMeeting {
    pub(crate) meeting_id: Uuid,
    pub(crate) user_id: Uuid,
    pub(crate) zoom_id: String,
    pub(crate) zoom_uuid: String,
    pub(crate) start_time: DateTime<Utc>,
    pub(crate) end_time: Option<DateTime<Utc>>,
    pub(crate) max_meeting_length_minutes: Option<i32>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

impl DBMeeting {
    pub(crate) fn is_ended(&self) -> bool {
        self.end_time.is_some()
    }

    pub(crate) fn duration(&self) -> chrono::Duration {
        let end_time_for_calc = self.end_time.unwrap_or_else(chrono::Utc::now);

        end_time_for_calc - self.start_time
    }

    pub(crate) async fn fetch_minutes_remaining(&self, app_state: &AppState) -> cja::Result<i32> {
        let user = sqlx::query_as!(
            DBUser,
            "SELECT * from Users where user_id = $1",
            self.user_id
        )
        .fetch_one(app_state.db())
        .await?;

        Ok(self.minutes_remaining(&user))
    }

    pub(crate) fn minutes_remaining(&self, user: &DBUser) -> i32 {
        let duration = self.duration();
        let max_duration = self.max_duration(user);

        (max_duration - duration).num_minutes() as i32
    }

    pub(crate) fn max_duration(&self, user: &DBUser) -> chrono::Duration {
        if let Some(max_meeting_length_minutes) = self.max_meeting_length_minutes {
            return chrono::Duration::minutes(max_meeting_length_minutes as i64);
        }

        if let Some(user_max_length) = user.default_meeting_length_minutes {
            return chrono::Duration::minutes(user_max_length as i64);
        }

        chrono::Duration::minutes(
            crate::jobs::end_meeting::DEFAULT_MAX_MEETING_LENGTH_MINUTES as i64,
        )
    }
}

pub struct DBUser {
    pub(crate) user_id: Uuid,
    #[allow(dead_code)]
    pub(crate) zoom_id: String,
    pub(crate) display_name: String,
    pub(crate) access_token: String,
    pub(crate) refresh_token: String,
    pub(crate) expires_at: DateTime<Utc>,
    pub(crate) default_meeting_length_minutes: Option<i32>,
    #[allow(dead_code)]
    pub(crate) created_at: DateTime<Utc>,
    #[allow(dead_code)]
    pub(crate) updated_at: DateTime<Utc>,
}

impl DBUser {
    pub(crate) fn is_access_token_expired(&self) -> bool {
        let now_with_buffer = chrono::Utc::now() + chrono::Duration::seconds(60);

        self.expires_at < now_with_buffer
    }

    pub(crate) async fn access_token(&self, app_state: &AppState) -> cja::Result<String> {
        if !self.is_access_token_expired() {
            return Ok(self.access_token.clone());
        }

        let token_response =
            crate::zoom::refresh_access_token(&app_state.zoom, &self.refresh_token).await?;

        let expires_at = chrono::Utc::now() + chrono::Duration::seconds(token_response.expires_in);

        sqlx::query!(
            "UPDATE users SET access_token = $1, expires_at = $2 WHERE user_id = $3",
            token_response.access_token,
            expires_at,
            self.user_id
        )
        .execute(&app_state.db)
        .await?;

        Ok(token_response.access_token)
    }
}
