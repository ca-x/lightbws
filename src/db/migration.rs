use sea_orm_migration::prelude::*;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(InitialMigration)]
    }
}

struct InitialMigration;

impl MigrationName for InitialMigration {
    fn name(&self) -> &str {
        "m20260729_000001_initial"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for InitialMigration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE TABLE users (
                    id TEXT PRIMARY KEY NOT NULL,
                    username VARCHAR(128) NOT NULL COLLATE NOCASE UNIQUE,
                    display_name VARCHAR(128) NOT NULL,
                    role TEXT NOT NULL CHECK(role IN ('admin','user')),
                    password_hash TEXT NOT NULL,
                    disabled BOOLEAN NOT NULL DEFAULT 0,
                    created_at BIGINT NOT NULL,
                    updated_at BIGINT NOT NULL,
                    last_login_at BIGINT
                );
                CREATE TABLE sessions (
                    id TEXT PRIMARY KEY NOT NULL,
                    user_id TEXT NOT NULL,
                    csrf_digest TEXT NOT NULL,
                    expires_at BIGINT NOT NULL,
                    created_at BIGINT NOT NULL,
                    CONSTRAINT fk_session_user FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
                );
                CREATE INDEX idx_sessions_user_expiry ON sessions(user_id, expires_at);
                CREATE TABLE projects (
                    id TEXT PRIMARY KEY NOT NULL,
                    organization_id TEXT NOT NULL,
                    name_cipher TEXT,
                    name_plain VARCHAR(500),
                    deleted_at BIGINT,
                    created_at BIGINT NOT NULL,
                    updated_at BIGINT NOT NULL,
                    revision_nanos BIGINT NOT NULL,
                    CHECK ((name_cipher IS NOT NULL AND name_plain IS NULL)
                        OR (name_cipher IS NULL AND name_plain IS NOT NULL))
                );
                CREATE INDEX idx_projects_org_deleted ON projects(organization_id, deleted_at, updated_at);
                CREATE TABLE secrets (
                    id TEXT PRIMARY KEY NOT NULL,
                    organization_id TEXT NOT NULL,
                    project_id TEXT,
                    key_cipher TEXT,
                    value_cipher TEXT,
                    note_cipher TEXT,
                    key_plain VARCHAR(500),
                    value_plain TEXT,
                    note_plain TEXT,
                    deleted_at BIGINT,
                    created_at BIGINT NOT NULL,
                    updated_at BIGINT NOT NULL,
                    revision_nanos BIGINT NOT NULL,
                    CHECK ((key_cipher IS NOT NULL AND value_cipher IS NOT NULL AND note_cipher IS NOT NULL
                            AND key_plain IS NULL AND value_plain IS NULL AND note_plain IS NULL)
                        OR (key_cipher IS NULL AND value_cipher IS NULL AND note_cipher IS NULL
                            AND key_plain IS NOT NULL AND value_plain IS NOT NULL AND note_plain IS NOT NULL)),
                    CONSTRAINT fk_secret_project FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE SET NULL
                );
                CREATE INDEX idx_secrets_org_deleted ON secrets(organization_id, deleted_at, updated_at);
                CREATE INDEX idx_secrets_project ON secrets(project_id, deleted_at);
                CREATE TABLE machine_accounts (
                    id TEXT PRIMARY KEY NOT NULL,
                    name VARCHAR(128) NOT NULL UNIQUE,
                    client_id TEXT NOT NULL UNIQUE,
                    client_secret_digest TEXT NOT NULL,
                    created_by TEXT NOT NULL,
                    last_used_at BIGINT,
                    revoked_at BIGINT,
                    created_at BIGINT NOT NULL,
                    CONSTRAINT fk_machine_creator FOREIGN KEY(created_by) REFERENCES users(id) ON DELETE RESTRICT
                );
                CREATE TABLE machine_sessions (
                    id TEXT PRIMARY KEY NOT NULL,
                    machine_account_id TEXT NOT NULL,
                    expires_at BIGINT NOT NULL,
                    created_at BIGINT NOT NULL,
                    CONSTRAINT fk_machine_session_account FOREIGN KEY(machine_account_id) REFERENCES machine_accounts(id) ON DELETE CASCADE
                );
                CREATE INDEX idx_machine_sessions_account_expiry ON machine_sessions(machine_account_id, expires_at);
                CREATE TABLE sdk_sync_state (
                    organization_id TEXT PRIMARY KEY NOT NULL,
                    revision_nanos BIGINT NOT NULL
                );
                INSERT INTO sdk_sync_state (organization_id, revision_nanos)
                VALUES ('f4e44a7f-1190-432a-9d4a-af96013127cb', 0);
                CREATE TABLE audit_events (
                    id TEXT PRIMARY KEY NOT NULL,
                    actor_user_id TEXT,
                    action VARCHAR(100) NOT NULL,
                    resource_kind VARCHAR(64) NOT NULL,
                    resource_id TEXT,
                    created_at BIGINT NOT NULL,
                    CONSTRAINT fk_audit_actor FOREIGN KEY(actor_user_id) REFERENCES users(id) ON DELETE SET NULL
                );
                CREATE INDEX idx_audit_created ON audit_events(created_at DESC);
                CREATE TABLE backup_targets (
                    id TEXT PRIMARY KEY NOT NULL,
                    display_name VARCHAR(128) NOT NULL UNIQUE,
                    kind TEXT NOT NULL CHECK(kind IN ('s3','webdav')),
                    public_config_json TEXT NOT NULL,
                    credentials_cipher TEXT NOT NULL,
                    enabled BOOLEAN NOT NULL DEFAULT 1,
                    schedule_enabled BOOLEAN NOT NULL DEFAULT 0,
                    interval_hours INTEGER NOT NULL DEFAULT 24,
                    next_run_at BIGINT,
                    last_run_at BIGINT,
                    last_status TEXT,
                    last_error TEXT,
                    created_at BIGINT NOT NULL,
                    updated_at BIGINT NOT NULL
                );
                CREATE TABLE backup_jobs (
                    id TEXT PRIMARY KEY NOT NULL,
                    target_id TEXT NOT NULL,
                    trigger_kind TEXT NOT NULL CHECK(trigger_kind IN ('manual','scheduled')),
                    status TEXT NOT NULL CHECK(status IN ('running','succeeded','failed')),
                    object_key TEXT NOT NULL,
                    byte_size BIGINT,
                    error_code TEXT,
                    created_at BIGINT NOT NULL,
                    completed_at BIGINT,
                    CONSTRAINT fk_backup_target FOREIGN KEY(target_id) REFERENCES backup_targets(id) ON DELETE CASCADE
                );
                CREATE INDEX idx_backup_jobs_created ON backup_jobs(created_at DESC);
                CREATE UNIQUE INDEX idx_backup_jobs_one_running
                ON backup_jobs(target_id) WHERE status = 'running';
                "#,
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                DROP TABLE IF EXISTS backup_jobs;
                DROP TABLE IF EXISTS backup_targets;
                DROP TABLE IF EXISTS audit_events;
                DROP TABLE IF EXISTS sdk_sync_state;
                DROP TABLE IF EXISTS machine_sessions;
                DROP TABLE IF EXISTS machine_accounts;
                DROP TABLE IF EXISTS secrets;
                DROP TABLE IF EXISTS projects;
                DROP TABLE IF EXISTS sessions;
                DROP TABLE IF EXISTS users;
                "#,
            )
            .await?;
        Ok(())
    }
}
