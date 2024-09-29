use cja::jobs::Job;
use serde::{Deserialize, Serialize};

use crate::AppState;

#[derive(Debug, Clone, Deserialize, Serialize)]
struct NoopJob;

#[async_trait::async_trait]
impl Job<AppState> for NoopJob {
    const NAME: &'static str = "NoopJob";

    async fn run(&self, _app_state: AppState) -> cja::Result<()> {
        Ok(())
    }
}

pub(crate) mod end_meeting;

pub(crate) mod check_live_meetings;

cja::impl_job_registry!(
    crate::AppState,
    NoopJob,
    end_meeting::EndActiveMeetings,
    end_meeting::EndMeeting,
    check_live_meetings::CheckLiveUserMeetings,
    check_live_meetings::CheckLiveMeetings
);
