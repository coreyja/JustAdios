use cja::{app_state::AppState as AS, color_eyre::eyre::Context as _, server::run_server};
use db::setup_db_pool;
use tracing::info;

mod cron;
mod db;
mod jobs;
mod routes;

#[derive(Clone, Debug)]
struct AppState {
    db: sqlx::PgPool,
    cookie_key: cja::server::cookies::CookieKey,
}

impl AS for AppState {
    fn db(&self) -> &sqlx::PgPool {
        &self.db
    }

    fn version(&self) -> &str {
        "dev"
    }

    fn cookie_key(&self) -> &cja::server::cookies::CookieKey {
        &self.cookie_key
    }
}

fn main() -> cja::Result<()> {
    let _sentry_guard = cja::setup::setup_sentry();

    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()?
        .block_on(_main())
}

async fn _main() -> cja::Result<()> {
    cja::setup::setup_tracing("just-adios")?;

    let db_pool = setup_db_pool().await.context("Failed to setup DB Pool")?;

    let cookie_key = cja::server::cookies::CookieKey::from_env_or_generate()?;

    let app_state = AppState {
        db: db_pool,
        cookie_key,
    };

    let app = routes::routes(app_state.clone());

    info!("Spawning Tasks");
    let mut futures = vec![
        tokio::spawn(run_server(app)),
        tokio::spawn(cja::jobs::worker::job_worker(app_state.clone(), jobs::Jobs)),
    ];
    if std::env::var("CRON_DISABLED").unwrap_or_else(|_| "false".to_string()) != "true" {
        info!("Cron Enabled");
        futures.push(tokio::spawn(cron::run_cron(app_state.clone())));
    }
    info!("Tasks Spawned");

    futures::future::try_join_all(futures).await?;

    Ok(())
}
