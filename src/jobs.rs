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

cja::impl_job_registry!(crate::AppState, NoopJob);
