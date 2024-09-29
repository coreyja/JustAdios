use chrono::Utc;
use cja::{jobs::Job, uuid::Uuid};
use serde::{Deserialize, Serialize};

use crate::{
    db::DBUser,
    zoom::{get_meetings, MeetingType},
    AppState,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct UserId(Uuid);

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct CheckLiveUserMeetings(UserId);

#[async_trait::async_trait]
impl Job<AppState> for CheckLiveUserMeetings {
    const NAME: &'static str = "CheckLiveUserMeetings";

    async fn run(&self, app_state: AppState) -> cja::Result<()> {
        let user_id = self.0 .0;
        let user = sqlx::query_as!(DBUser, "SELECT * FROM users WHERE user_id = $1", user_id)
            .fetch_one(&app_state.db)
            .await?;

        let meetings = get_meetings(&user.access_token, MeetingType::Live).await?;
        for meeting in meetings.meetings.iter() {
            let start_time = Utc::now();
            sqlx::query!(
              "INSERT INTO meetings (user_id, zoom_id, zoom_uuid, start_time) VALUES ($1, $2, $3, $4) ON CONFLICT (zoom_id) DO NOTHING",
              user_id,
              meeting.id.to_string(),
              meeting.uuid,
              start_time,
            )
            .execute(&app_state.db)
            .await?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct CheckLiveMeetings;

#[async_trait::async_trait]
impl Job<AppState> for CheckLiveMeetings {
    const NAME: &'static str = "CheckLiveMeetings";

    async fn run(&self, app_state: AppState) -> cja::Result<()> {
        let users = sqlx::query_as!(DBUser, "SELECT * FROM users")
            .fetch_all(&app_state.db)
            .await?;

        for user in users.iter() {
            CheckLiveUserMeetings(UserId(user.user_id))
                .enqueue(app_state.clone(), "CheckLiveMeetings Loop".to_string())
                .await?;
        }

        Ok(())
    }
}
