use sea_orm_migration::prelude::*;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(InitialMigration), Box::new(BackupOptionsMigration)]
    }
}

struct BackupOptionsMigration;

impl MigrationName for BackupOptionsMigration {
    fn name(&self) -> &str {
        "m20260729_000002_backup_options"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for BackupOptionsMigration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                ALTER TABLE backup_targets
                ADD COLUMN scopes_json TEXT NOT NULL DEFAULT '{}'
                CHECK(json_valid(scopes_json));
                ALTER TABLE backup_targets
                ADD COLUMN encryption_mode TEXT NOT NULL DEFAULT 'master_key'
                CHECK(encryption_mode IN ('master_key','plaintext'));
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
                ALTER TABLE backup_targets DROP COLUMN encryption_mode;
                ALTER TABLE backup_targets DROP COLUMN scopes_json;
                "#,
            )
            .await?;
        Ok(())
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
                CREATE TABLE organizations (
                    id TEXT PRIMARY KEY NOT NULL,
                    name VARCHAR(128) NOT NULL,
                    CHECK(length(id) = 36),
                    CHECK(length(trim(name)) BETWEEN 1 AND 128)
                );
                INSERT INTO organizations (id, name)
                VALUES ('f4e44a7f-1190-432a-9d4a-af96013127cb', 'LightBWS');
                CREATE TABLE users (
                    id TEXT PRIMARY KEY NOT NULL,
                    username VARCHAR(128) NOT NULL COLLATE NOCASE UNIQUE,
                    display_name VARCHAR(128) NOT NULL,
                    role TEXT NOT NULL CHECK(role IN ('admin','user')),
                    password_hash TEXT NOT NULL,
                    disabled BOOLEAN NOT NULL DEFAULT 0,
                    created_at BIGINT NOT NULL,
                    updated_at BIGINT NOT NULL,
                    last_login_at BIGINT,
                    CHECK(length(id) = 36),
                    CHECK(length(username) BETWEEN 1 AND 128 AND username = trim(username)),
                    CHECK(length(display_name) BETWEEN 1 AND 128 AND display_name = trim(display_name)),
                    CHECK(length(password_hash) BETWEEN 1 AND 1024),
                    CHECK(disabled IN (0, 1)),
                    CHECK(created_at >= 0),
                    CHECK(updated_at >= created_at),
                    CHECK(last_login_at IS NULL OR last_login_at >= created_at)
                );
                CREATE INDEX idx_users_role_disabled ON users(role, disabled);
                CREATE TABLE sessions (
                    id TEXT PRIMARY KEY NOT NULL,
                    user_id TEXT NOT NULL,
                    csrf_digest TEXT NOT NULL,
                    expires_at BIGINT NOT NULL,
                    created_at BIGINT NOT NULL,
                    CHECK(length(id) = 64),
                    CHECK(length(csrf_digest) = 64),
                    CHECK(created_at >= 0),
                    CHECK(expires_at > created_at),
                    CONSTRAINT fk_session_user FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
                );
                CREATE INDEX idx_sessions_user_expiry ON sessions(user_id, expires_at);
                CREATE INDEX idx_sessions_expiry ON sessions(expires_at);
                CREATE TABLE projects (
                    id TEXT PRIMARY KEY NOT NULL,
                    organization_id TEXT NOT NULL,
                    name_cipher TEXT,
                    name_plain VARCHAR(500),
                    deleted_at BIGINT,
                    created_at BIGINT NOT NULL,
                    updated_at BIGINT NOT NULL,
                    revision_nanos BIGINT NOT NULL,
                    CHECK(length(id) = 36),
                    CHECK(length(organization_id) = 36),
                    CHECK ((name_cipher IS NOT NULL AND name_plain IS NULL)
                        OR (name_cipher IS NULL AND name_plain IS NOT NULL)),
                    CHECK(name_cipher IS NULL OR length(name_cipher) BETWEEN 1 AND 16384),
                    CHECK(name_plain IS NULL OR length(trim(name_plain)) BETWEEN 1 AND 500),
                    CHECK(created_at >= 0),
                    CHECK(updated_at >= created_at),
                    CHECK(deleted_at IS NULL OR deleted_at BETWEEN created_at AND updated_at),
                    CHECK(revision_nanos >= 0),
                    CONSTRAINT fk_project_organization FOREIGN KEY(organization_id) REFERENCES organizations(id) ON DELETE RESTRICT
                );
                CREATE INDEX idx_projects_org_deleted_updated ON projects(organization_id, deleted_at, updated_at DESC);
                CREATE TABLE secrets (
                    id TEXT PRIMARY KEY NOT NULL,
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
                    CHECK(length(id) = 36),
                    CHECK(project_id IS NULL OR length(project_id) = 36),
                    CHECK ((key_cipher IS NOT NULL AND value_cipher IS NOT NULL AND note_cipher IS NOT NULL
                            AND key_plain IS NULL AND value_plain IS NULL AND note_plain IS NULL)
                        OR (key_cipher IS NULL AND value_cipher IS NULL AND note_cipher IS NULL
                            AND key_plain IS NOT NULL AND value_plain IS NOT NULL AND note_plain IS NOT NULL)),
                    CHECK(key_cipher IS NULL OR project_id IS NOT NULL),
                    CHECK(key_cipher IS NULL OR length(key_cipher) BETWEEN 1 AND 32768),
                    CHECK(value_cipher IS NULL OR length(value_cipher) BETWEEN 1 AND 2097152),
                    CHECK(note_cipher IS NULL OR length(note_cipher) <= 131072),
                    CHECK(key_plain IS NULL OR length(trim(key_plain)) BETWEEN 1 AND 500),
                    CHECK(value_plain IS NULL OR length(value_plain) <= 1048576),
                    CHECK(note_plain IS NULL OR length(note_plain) <= 65536),
                    CHECK(created_at >= 0),
                    CHECK(updated_at >= created_at),
                    CHECK(deleted_at IS NULL OR deleted_at BETWEEN created_at AND updated_at),
                    CHECK(revision_nanos >= 0),
                    CONSTRAINT fk_secret_project FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE SET NULL
                );
                CREATE INDEX idx_secrets_deleted_updated ON secrets(deleted_at, updated_at DESC);
                CREATE INDEX idx_secrets_project_deleted_updated ON secrets(project_id, deleted_at, updated_at DESC);
                CREATE INDEX idx_secrets_revision ON secrets(revision_nanos DESC);
                CREATE TABLE machine_accounts (
                    id TEXT PRIMARY KEY NOT NULL,
                    name VARCHAR(128) NOT NULL COLLATE NOCASE UNIQUE,
                    client_id TEXT NOT NULL UNIQUE,
                    created_by TEXT NOT NULL,
                    last_used_at BIGINT,
                    revoked_at BIGINT,
                    compatibility_account BOOLEAN NOT NULL DEFAULT 0,
                    created_at BIGINT NOT NULL,
                    CHECK(length(id) = 36),
                    CHECK(length(name) BETWEEN 1 AND 128 AND name = trim(name)),
                    CHECK(length(client_id) = 36),
                    CHECK(length(created_by) = 36),
                    CHECK(compatibility_account IN (0, 1)),
                    CHECK(created_at >= 0),
                    CHECK(last_used_at IS NULL OR last_used_at >= created_at),
                    CHECK(revoked_at IS NULL OR revoked_at >= created_at),
                    CONSTRAINT fk_machine_creator FOREIGN KEY(created_by) REFERENCES users(id) ON DELETE RESTRICT
                );
                CREATE UNIQUE INDEX idx_machine_accounts_one_compatibility
                ON machine_accounts(compatibility_account) WHERE compatibility_account = 1;
                CREATE TABLE machine_access_tokens (
                    id TEXT PRIMARY KEY NOT NULL,
                    machine_account_id TEXT NOT NULL,
                    name VARCHAR(128) NOT NULL COLLATE NOCASE,
                    secret_digest TEXT NOT NULL,
                    expires_at BIGINT,
                    last_used_at BIGINT,
                    revoked_at BIGINT,
                    created_at BIGINT NOT NULL,
                    CHECK(length(id) = 36),
                    CHECK(length(machine_account_id) = 36),
                    CHECK(length(name) BETWEEN 1 AND 128 AND name = trim(name)),
                    CHECK(length(secret_digest) = 64),
                    CHECK(created_at >= 0),
                    CHECK(expires_at IS NULL OR expires_at > created_at),
                    CHECK(last_used_at IS NULL OR last_used_at >= created_at),
                    CHECK(revoked_at IS NULL OR revoked_at >= created_at),
                    CONSTRAINT fk_machine_access_token_account FOREIGN KEY(machine_account_id) REFERENCES machine_accounts(id) ON DELETE CASCADE
                );
                CREATE UNIQUE INDEX idx_machine_access_tokens_account_name_active
                ON machine_access_tokens(machine_account_id, name) WHERE revoked_at IS NULL;
                CREATE UNIQUE INDEX idx_machine_access_tokens_account_digest
                ON machine_access_tokens(machine_account_id, secret_digest);
                CREATE INDEX idx_machine_access_tokens_account_status
                ON machine_access_tokens(machine_account_id, revoked_at, expires_at, created_at DESC);
                CREATE TABLE machine_sessions (
                    id TEXT PRIMARY KEY NOT NULL,
                    machine_access_token_id TEXT NOT NULL,
                    expires_at BIGINT NOT NULL,
                    created_at BIGINT NOT NULL,
                    CHECK(length(id) = 64),
                    CHECK(length(machine_access_token_id) = 36),
                    CHECK(created_at >= 0),
                    CHECK(expires_at > created_at),
                    CONSTRAINT fk_machine_session_access_token FOREIGN KEY(machine_access_token_id) REFERENCES machine_access_tokens(id) ON DELETE CASCADE
                );
                CREATE INDEX idx_machine_sessions_token_expiry ON machine_sessions(machine_access_token_id, expires_at);
                CREATE INDEX idx_machine_sessions_expiry ON machine_sessions(expires_at);
                CREATE TABLE groups (
                    id TEXT PRIMARY KEY NOT NULL,
                    name VARCHAR(128) NOT NULL COLLATE NOCASE UNIQUE,
                    created_at BIGINT NOT NULL,
                    updated_at BIGINT NOT NULL,
                    CHECK(length(id) = 36),
                    CHECK(length(name) BETWEEN 1 AND 128 AND name = trim(name)),
                    CHECK(created_at >= 0),
                    CHECK(updated_at >= created_at)
                );
                CREATE TABLE group_members (
                    group_id TEXT NOT NULL,
                    user_id TEXT NOT NULL,
                    created_at BIGINT NOT NULL CHECK(created_at >= 0),
                    PRIMARY KEY(group_id, user_id),
                    CONSTRAINT fk_group_member_group FOREIGN KEY(group_id) REFERENCES groups(id) ON DELETE CASCADE,
                    CONSTRAINT fk_group_member_user FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
                ) WITHOUT ROWID;
                CREATE INDEX idx_group_members_user ON group_members(user_id, group_id);
                CREATE TABLE project_user_grants (
                    project_id TEXT NOT NULL,
                    user_id TEXT NOT NULL,
                    can_read BOOLEAN NOT NULL,
                    can_write BOOLEAN NOT NULL,
                    created_at BIGINT NOT NULL,
                    updated_at BIGINT NOT NULL,
                    PRIMARY KEY(project_id, user_id),
                    CHECK(can_read = 1),
                    CHECK(can_write IN (0, 1)),
                    CHECK(created_at >= 0),
                    CHECK(updated_at >= created_at),
                    CONSTRAINT fk_project_user_grant_project FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE,
                    CONSTRAINT fk_project_user_grant_user FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
                ) WITHOUT ROWID;
                CREATE INDEX idx_project_user_grants_user ON project_user_grants(user_id, project_id);
                CREATE TABLE project_group_grants (
                    project_id TEXT NOT NULL,
                    group_id TEXT NOT NULL,
                    can_read BOOLEAN NOT NULL,
                    can_write BOOLEAN NOT NULL,
                    created_at BIGINT NOT NULL,
                    updated_at BIGINT NOT NULL,
                    PRIMARY KEY(project_id, group_id),
                    CHECK(can_read = 1),
                    CHECK(can_write IN (0, 1)),
                    CHECK(created_at >= 0),
                    CHECK(updated_at >= created_at),
                    CONSTRAINT fk_project_group_grant_project FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE,
                    CONSTRAINT fk_project_group_grant_group FOREIGN KEY(group_id) REFERENCES groups(id) ON DELETE CASCADE
                ) WITHOUT ROWID;
                CREATE INDEX idx_project_group_grants_group ON project_group_grants(group_id, project_id);
                CREATE TABLE project_machine_grants (
                    project_id TEXT NOT NULL,
                    machine_account_id TEXT NOT NULL,
                    can_read BOOLEAN NOT NULL,
                    can_write BOOLEAN NOT NULL,
                    created_at BIGINT NOT NULL,
                    updated_at BIGINT NOT NULL,
                    PRIMARY KEY(project_id, machine_account_id),
                    CHECK(can_read = 1),
                    CHECK(can_write IN (0, 1)),
                    CHECK(created_at >= 0),
                    CHECK(updated_at >= created_at),
                    CONSTRAINT fk_project_machine_grant_project FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE,
                    CONSTRAINT fk_project_machine_grant_machine FOREIGN KEY(machine_account_id) REFERENCES machine_accounts(id) ON DELETE CASCADE
                ) WITHOUT ROWID;
                CREATE INDEX idx_project_machine_grants_machine ON project_machine_grants(machine_account_id, project_id);
                CREATE TABLE secret_user_grants (
                    secret_id TEXT NOT NULL,
                    user_id TEXT NOT NULL,
                    can_read BOOLEAN NOT NULL,
                    can_write BOOLEAN NOT NULL,
                    created_at BIGINT NOT NULL,
                    updated_at BIGINT NOT NULL,
                    PRIMARY KEY(secret_id, user_id),
                    CHECK(can_read = 1),
                    CHECK(can_write IN (0, 1)),
                    CHECK(created_at >= 0),
                    CHECK(updated_at >= created_at),
                    CONSTRAINT fk_secret_user_grant_secret FOREIGN KEY(secret_id) REFERENCES secrets(id) ON DELETE CASCADE,
                    CONSTRAINT fk_secret_user_grant_user FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
                ) WITHOUT ROWID;
                CREATE INDEX idx_secret_user_grants_user ON secret_user_grants(user_id, secret_id);
                CREATE TABLE secret_group_grants (
                    secret_id TEXT NOT NULL,
                    group_id TEXT NOT NULL,
                    can_read BOOLEAN NOT NULL,
                    can_write BOOLEAN NOT NULL,
                    created_at BIGINT NOT NULL,
                    updated_at BIGINT NOT NULL,
                    PRIMARY KEY(secret_id, group_id),
                    CHECK(can_read = 1),
                    CHECK(can_write IN (0, 1)),
                    CHECK(created_at >= 0),
                    CHECK(updated_at >= created_at),
                    CONSTRAINT fk_secret_group_grant_secret FOREIGN KEY(secret_id) REFERENCES secrets(id) ON DELETE CASCADE,
                    CONSTRAINT fk_secret_group_grant_group FOREIGN KEY(group_id) REFERENCES groups(id) ON DELETE CASCADE
                ) WITHOUT ROWID;
                CREATE INDEX idx_secret_group_grants_group ON secret_group_grants(group_id, secret_id);
                CREATE TABLE secret_machine_grants (
                    secret_id TEXT NOT NULL,
                    machine_account_id TEXT NOT NULL,
                    can_read BOOLEAN NOT NULL,
                    can_write BOOLEAN NOT NULL,
                    created_at BIGINT NOT NULL,
                    updated_at BIGINT NOT NULL,
                    PRIMARY KEY(secret_id, machine_account_id),
                    CHECK(can_read = 1),
                    CHECK(can_write IN (0, 1)),
                    CHECK(created_at >= 0),
                    CHECK(updated_at >= created_at),
                    CONSTRAINT fk_secret_machine_grant_secret FOREIGN KEY(secret_id) REFERENCES secrets(id) ON DELETE CASCADE,
                    CONSTRAINT fk_secret_machine_grant_machine FOREIGN KEY(machine_account_id) REFERENCES machine_accounts(id) ON DELETE CASCADE
                ) WITHOUT ROWID;
                CREATE INDEX idx_secret_machine_grants_machine ON secret_machine_grants(machine_account_id, secret_id);
                CREATE TABLE machine_user_grants (
                    machine_account_id TEXT NOT NULL,
                    user_id TEXT NOT NULL,
                    can_read BOOLEAN NOT NULL,
                    can_write BOOLEAN NOT NULL,
                    created_at BIGINT NOT NULL,
                    updated_at BIGINT NOT NULL,
                    PRIMARY KEY(machine_account_id, user_id),
                    CHECK(can_read = 1),
                    CHECK(can_write IN (0, 1)),
                    CHECK(created_at >= 0),
                    CHECK(updated_at >= created_at),
                    CONSTRAINT fk_machine_user_grant_machine FOREIGN KEY(machine_account_id) REFERENCES machine_accounts(id) ON DELETE CASCADE,
                    CONSTRAINT fk_machine_user_grant_user FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
                ) WITHOUT ROWID;
                CREATE INDEX idx_machine_user_grants_user ON machine_user_grants(user_id, machine_account_id);
                CREATE TABLE machine_group_grants (
                    machine_account_id TEXT NOT NULL,
                    group_id TEXT NOT NULL,
                    can_read BOOLEAN NOT NULL,
                    can_write BOOLEAN NOT NULL,
                    created_at BIGINT NOT NULL,
                    updated_at BIGINT NOT NULL,
                    PRIMARY KEY(machine_account_id, group_id),
                    CHECK(can_read = 1),
                    CHECK(can_write IN (0, 1)),
                    CHECK(created_at >= 0),
                    CHECK(updated_at >= created_at),
                    CONSTRAINT fk_machine_group_grant_machine FOREIGN KEY(machine_account_id) REFERENCES machine_accounts(id) ON DELETE CASCADE,
                    CONSTRAINT fk_machine_group_grant_group FOREIGN KEY(group_id) REFERENCES groups(id) ON DELETE CASCADE
                ) WITHOUT ROWID;
                CREATE INDEX idx_machine_group_grants_group ON machine_group_grants(group_id, machine_account_id);
                CREATE TABLE sdk_sync_state (
                    organization_id TEXT PRIMARY KEY NOT NULL,
                    revision_nanos BIGINT NOT NULL CHECK(revision_nanos >= 0),
                    CONSTRAINT fk_sync_organization FOREIGN KEY(organization_id) REFERENCES organizations(id) ON DELETE CASCADE
                ) WITHOUT ROWID;
                INSERT INTO sdk_sync_state (organization_id, revision_nanos)
                VALUES ('f4e44a7f-1190-432a-9d4a-af96013127cb', 0);
                CREATE TRIGGER trg_sdk_sync_revision_monotonic
                BEFORE UPDATE OF revision_nanos ON sdk_sync_state
                FOR EACH ROW WHEN NEW.revision_nanos <= OLD.revision_nanos
                BEGIN
                    SELECT RAISE(ABORT, 'SDK revision must increase');
                END;
                CREATE TABLE audit_settings (
                    id INTEGER PRIMARY KEY NOT NULL CHECK(id = 1),
                    enabled BOOLEAN NOT NULL DEFAULT 1 CHECK(enabled IN (0, 1)),
                    auto_cleanup_enabled BOOLEAN NOT NULL DEFAULT 1 CHECK(auto_cleanup_enabled IN (0, 1)),
                    retention_days INTEGER NOT NULL DEFAULT 90 CHECK(retention_days BETWEEN 1 AND 3650),
                    cleanup_authorized BOOLEAN NOT NULL DEFAULT 0 CHECK(cleanup_authorized IN (0, 1)),
                    last_cleanup_at BIGINT,
                    created_at BIGINT NOT NULL,
                    updated_at BIGINT NOT NULL,
                    CHECK(created_at >= 0),
                    CHECK(updated_at >= created_at),
                    CHECK(last_cleanup_at IS NULL OR last_cleanup_at >= created_at)
                );
                INSERT INTO audit_settings (
                    id, enabled, auto_cleanup_enabled, retention_days, cleanup_authorized,
                    last_cleanup_at, created_at, updated_at
                ) VALUES (1, 1, 1, 90, 0, NULL, unixepoch(), unixepoch());
                CREATE TABLE audit_events (
                    id TEXT PRIMARY KEY NOT NULL,
                    actor_kind TEXT NOT NULL CHECK(actor_kind IN ('user','machine','system')),
                    actor_id TEXT,
                    action VARCHAR(100) NOT NULL,
                    resource_kind VARCHAR(64) NOT NULL,
                    resource_id TEXT,
                    outcome TEXT NOT NULL CHECK(outcome IN ('allowed','denied','changed')),
                    created_at BIGINT NOT NULL,
                    CHECK(length(id) = 36),
                    CHECK((actor_kind = 'system' AND actor_id IS NULL)
                        OR (actor_kind IN ('user','machine') AND actor_id IS NOT NULL
                            AND length(actor_id) = 36)),
                    CHECK(length(action) BETWEEN 1 AND 100),
                    CHECK(length(resource_kind) BETWEEN 1 AND 64),
                    CHECK(resource_id IS NULL OR length(resource_id) = 36),
                    CHECK(created_at >= 0)
                );
                CREATE INDEX idx_audit_created ON audit_events(created_at DESC, id DESC);
                CREATE INDEX idx_audit_actor_created ON audit_events(actor_kind, actor_id, created_at DESC);
                CREATE INDEX idx_audit_resource_created ON audit_events(resource_kind, resource_id, created_at DESC);
                CREATE TRIGGER trg_audit_events_no_update
                BEFORE UPDATE ON audit_events
                BEGIN
                    SELECT RAISE(ABORT, 'audit events are immutable');
                END;
                CREATE TRIGGER trg_audit_events_no_delete
                BEFORE DELETE ON audit_events
                FOR EACH ROW WHEN COALESCE(
                    (SELECT cleanup_authorized FROM audit_settings WHERE id = 1), 0
                ) <> 1
                BEGIN
                    SELECT RAISE(ABORT, 'audit events are immutable');
                END;
                CREATE TABLE backup_targets (
                    id TEXT PRIMARY KEY NOT NULL,
                    display_name VARCHAR(128) NOT NULL COLLATE NOCASE UNIQUE,
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
                    updated_at BIGINT NOT NULL,
                    CHECK(length(id) = 36),
                    CHECK(length(display_name) BETWEEN 1 AND 128 AND display_name = trim(display_name)),
                    CHECK(length(public_config_json) BETWEEN 2 AND 65536),
                    CHECK(json_valid(public_config_json)),
                    CHECK(length(credentials_cipher) BETWEEN 1 AND 1048576),
                    CHECK(enabled IN (0, 1)),
                    CHECK(schedule_enabled IN (0, 1)),
                    CHECK(interval_hours BETWEEN 1 AND 8760),
                    CHECK(created_at >= 0),
                    CHECK(updated_at >= created_at),
                    CHECK(next_run_at IS NULL OR next_run_at >= created_at),
                    CHECK((enabled = 1 AND schedule_enabled = 1 AND next_run_at IS NOT NULL)
                        OR ((enabled = 0 OR schedule_enabled = 0) AND next_run_at IS NULL)),
                    CHECK((last_status IS NULL AND last_run_at IS NULL AND last_error IS NULL)
                        OR (last_status = 'succeeded' AND last_run_at IS NOT NULL AND last_error IS NULL)
                        OR (last_status = 'failed' AND last_run_at IS NOT NULL
                            AND last_error IS NOT NULL AND length(last_error) BETWEEN 1 AND 512)),
                    CHECK(last_run_at IS NULL OR last_run_at BETWEEN created_at AND updated_at)
                );
                CREATE INDEX idx_backup_targets_due ON backup_targets(next_run_at)
                WHERE enabled = 1 AND schedule_enabled = 1;
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
                    CHECK(length(id) = 36),
                    CHECK(length(target_id) = 36),
                    CHECK(length(object_key) BETWEEN 1 AND 1024),
                    CHECK(byte_size IS NULL OR byte_size >= 0),
                    CHECK(error_code IS NULL OR length(error_code) BETWEEN 1 AND 512),
                    CHECK(created_at >= 0),
                    CHECK(completed_at IS NULL OR completed_at >= created_at),
                    CHECK((status = 'running' AND completed_at IS NULL AND byte_size IS NULL AND error_code IS NULL)
                        OR (status = 'succeeded' AND completed_at IS NOT NULL AND byte_size IS NOT NULL AND error_code IS NULL)
                        OR (status = 'failed' AND completed_at IS NOT NULL AND error_code IS NOT NULL)),
                    CONSTRAINT fk_backup_target FOREIGN KEY(target_id) REFERENCES backup_targets(id) ON DELETE CASCADE
                );
                CREATE INDEX idx_backup_jobs_created ON backup_jobs(created_at DESC);
                CREATE INDEX idx_backup_jobs_target_created ON backup_jobs(target_id, created_at DESC);
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
                DROP TABLE IF EXISTS audit_settings;
                DROP TRIGGER IF EXISTS trg_sdk_sync_revision_monotonic;
                DROP TABLE IF EXISTS sdk_sync_state;
                DROP TABLE IF EXISTS machine_group_grants;
                DROP TABLE IF EXISTS machine_user_grants;
                DROP TABLE IF EXISTS secret_machine_grants;
                DROP TABLE IF EXISTS secret_group_grants;
                DROP TABLE IF EXISTS secret_user_grants;
                DROP TABLE IF EXISTS project_machine_grants;
                DROP TABLE IF EXISTS project_group_grants;
                DROP TABLE IF EXISTS project_user_grants;
                DROP TABLE IF EXISTS group_members;
                DROP TABLE IF EXISTS groups;
                DROP TABLE IF EXISTS machine_sessions;
                DROP TABLE IF EXISTS machine_access_tokens;
                DROP TABLE IF EXISTS machine_accounts;
                DROP TABLE IF EXISTS secrets;
                DROP TABLE IF EXISTS projects;
                DROP TABLE IF EXISTS sessions;
                DROP TABLE IF EXISTS users;
                DROP TABLE IF EXISTS organizations;
                "#,
            )
            .await?;
        Ok(())
    }
}
