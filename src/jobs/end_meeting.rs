use cja::{jobs::Job, uuid::Uuid};
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::db::{DBMeeting, DBUser};
use crate::{zoom, AppState};

#[derive(Debug, Clone, Deserialize, Serialize, Copy)]
pub(crate) struct MeetingId(Uuid);

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct EndMeeting(MeetingId);

pub const DEFAULT_MAX_MEETING_LENGTH_MINUTES: i32 = 40;

#[async_trait::async_trait]
impl Job<AppState> for EndMeeting {
    const NAME: &'static str = "EndMeeting";

    async fn run(&self, app_state: AppState) -> cja::Result<()> {
        let meeting_id = self.0;
        let meeting = sqlx::query_as!(
            DBMeeting,
            "SELECT * FROM meetings WHERE meeting_id = $1",
            meeting_id.0
        )
        .fetch_one(&app_state.db)
        .await?;

        let owner = sqlx::query_as!(
            DBUser,
            "SELECT * FROM users WHERE user_id = $1",
            meeting.user_id
        )
        .fetch_one(&app_state.db)
        .await?;

        if meeting.is_ended() {
            debug!("Meeting already ended");
            return Ok(());
        }

        let duration = meeting.duration();
        let max_duration = meeting.max_duration(&owner);

        if duration > max_duration {
            debug!("Meeting duration is long enough, going to end it");

            let access_token = owner.access_token(&app_state).await?;
            zoom::adios(&meeting.zoom_id, &access_token).await?;
        } else {
            debug!("Meeting duration is too short");
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct EndActiveMeetings;

#[async_trait::async_trait]
impl Job<AppState> for EndActiveMeetings {
    const NAME: &'static str = "EndActiveMeetings";

    async fn run(&self, app_state: AppState) -> cja::Result<()> {
        let meetings = sqlx::query_as!(DBMeeting, "SELECT * FROM meetings WHERE end_time is NULL")
            .fetch_all(&app_state.db)
            .await?;

        for meeting in meetings {
            EndMeeting(MeetingId(meeting.meeting_id))
                .enqueue(app_state.clone(), "EndActiveMeetingsLoop".to_string())
                .await?;
        }

        Ok(())
    }
}
