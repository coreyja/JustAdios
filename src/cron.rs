use std::time::Duration;

use cja::cron::{CronRegistry, Worker};

use crate::{
    jobs::{check_live_meetings::CheckLiveMeetings, end_meeting::EndActiveMeetings},
    AppState,
};

fn cron_registry() -> CronRegistry<AppState> {
    let mut registry = CronRegistry::new();
    registry.register_job(CheckLiveMeetings, Duration::from_secs(60 * 5));
    registry.register_job(EndActiveMeetings, Duration::from_secs(30));
    registry
}

pub(crate) async fn run_cron(app_state: AppState) -> cja::Result<()> {
    Ok(Worker::new(app_state, cron_registry()).run().await?)
}
