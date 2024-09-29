use chrono::{DateTime, Utc};
use cja::{
    color_eyre::{self, eyre::Context as _},
    uuid::Uuid,
};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, PgPool};

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
    pub(crate) id: Uuid,
    pub(crate) user_id: Uuid,
    pub(crate) zoom_id: String,
    pub(crate) zoom_uuid: String,
    pub(crate) start_time: DateTime<Utc>,
    pub(crate) end_time: Option<DateTime<Utc>>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}

pub struct DBUser {
    pub(crate) user_id: Uuid,
    pub(crate) zoom_id: String,
    pub(crate) display_name: String,
    pub(crate) access_token: String,
    pub(crate) refresh_token: String,
    pub(crate) expires_at: DateTime<Utc>,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
}
