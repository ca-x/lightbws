use anyhow::{Context, Result};
use lightbws::{
    AppState,
    config::Config,
    create_app,
    crypto::MasterKey,
    db::Database,
    domain::{machines::MachineRepository, users::UserRepository},
};
use tokio::net::TcpListener;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "lightbws=info,tower_http=info".into()),
        )
        .init();
    let config = Config::from_env()?;
    tokio::fs::create_dir_all(&config.data_dir)
        .await
        .with_context(|| format!("failed to create {}", config.data_dir.display()))?;
    let db = Database::connect(&config.database_path()).await?;
    let users = UserRepository::new(db.clone());
    users.bootstrap(config.bootstrap_admin.as_ref()).await?;
    if config.upstream_compatibility_account {
        let admin = users
            .list()
            .await?
            .into_iter()
            .find(|user| user.role == lightbws::domain::users::Role::Admin)
            .context("administrator is missing")?;
        MachineRepository::new(db.clone())
            .ensure_compatibility_account(admin.id)
            .await?;
        tracing::warn!("publicly known upstream SDK test credentials are enabled");
    }
    let master_key = MasterKey::load_or_create(&config)?;
    let state = AppState::new(db, &config).with_master_key(master_key);
    lightbws::domain::backups::recover_interrupted_jobs(&state.db).await?;
    tokio::spawn(lightbws::domain::backups::scheduler(state.clone()));
    let listener = TcpListener::bind(config.bind).await?;
    tracing::info!(address = %config.bind, "LightBWS is listening");
    axum::serve(
        listener,
        create_app(state).into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown())
    .await?;
    Ok(())
}

async fn shutdown() {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.expect("Ctrl-C handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = terminate => {} }
}
