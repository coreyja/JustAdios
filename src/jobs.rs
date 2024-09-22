use cja::{jobs::Job, uuid::Uuid};
use serde::{Deserialize, Serialize};

use crate::{
    zoom::{get_meetings, MeetingType},
    AppState, User,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
struct NoopJob;

#[async_trait::async_trait]
impl Job<AppState> for NoopJob {
    const NAME: &'static str = "NoopJob";

    async fn run(&self, _app_state: AppState) -> cja::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct UserId(Uuid);

#[derive(Debug, Clone, Deserialize, Serialize)]
struct CheckAndEndTooLongUserMeetings(UserId);

#[async_trait::async_trait]
impl Job<AppState> for CheckAndEndTooLongUserMeetings {
    const NAME: &'static str = "CheckAndEndTooLongUserMeetings";

    async fn run(&self, app_state: AppState) -> cja::Result<()> {
        let user_id = self.0 .0;
        let user = sqlx::query_as!(User, "SELECT * FROM users WHERE user_id = $1", user_id)
            .fetch_one(&app_state.db)
            .await?;

        let meetings = get_meetings(&user.access_token, MeetingType::Live).await?;
        for meeting in meetings.meetings.iter() {
            let duration = meeting.live_duration()?;
            if duration > 60 {
                meeting.adios(&user.access_token).await?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CheckAndEndTooLongMeetings;

#[async_trait::async_trait]
impl Job<AppState> for CheckAndEndTooLongMeetings {
    const NAME: &'static str = "CheckAndEndTooLongMeetings";

    async fn run(&self, app_state: AppState) -> cja::Result<()> {
        let users = sqlx::query_as!(User, "SELECT * FROM users")
            .fetch_all(&app_state.db)
            .await?;

        for user in users.iter() {
            CheckAndEndTooLongUserMeetings(UserId(user.user_id))
                .enqueue(
                    app_state.clone(),
                    "CronJob#CheckAndEndTooLongMeetings".to_string(),
                )
                .await?;
        }

        Ok(())
    }
}

cja::impl_job_registry!(
    crate::AppState,
    NoopJob,
    CheckAndEndTooLongUserMeetings,
    CheckAndEndTooLongMeetings
);
