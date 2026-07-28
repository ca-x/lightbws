pub mod entities;
mod migration;

use std::{path::Path, time::Duration};

use anyhow::{Context, Result};
use sea_orm::{
    ConnectOptions, Database as SeaDatabase, DatabaseConnection,
    sqlx::sqlite::{SqliteJournalMode, SqliteSynchronous},
};
use sea_orm_migration::MigratorTrait;

use self::migration::Migrator;

#[derive(Clone)]
pub struct Database {
    connection: DatabaseConnection,
}

impl Database {
    pub async fn connect(path: &Path) -> Result<Self> {
        let database_url = format!("sqlite://{}?mode=rwc", path.display());
        let mut options = ConnectOptions::new(database_url);
        options
            .max_connections(5)
            .min_connections(1)
            .connect_timeout(std::time::Duration::from_secs(10))
            .acquire_timeout(std::time::Duration::from_secs(10))
            .sqlx_logging(false);
        options.map_sqlx_sqlite_opts(|options| {
            options
                .foreign_keys(true)
                .journal_mode(SqliteJournalMode::Wal)
                .synchronous(SqliteSynchronous::Normal)
                .busy_timeout(Duration::from_secs(5))
        });
        let connection = SeaDatabase::connect(options)
            .await
            .with_context(|| format!("failed to open SQLite database at {}", path.display()))?;
        Migrator::up(&connection, None)
            .await
            .context("failed to migrate SQLite database")?;
        Ok(Self { connection })
    }

    pub fn connection(&self) -> &DatabaseConnection {
        &self.connection
    }
}
