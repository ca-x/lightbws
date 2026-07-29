use lightbws::{
    crypto::MasterKey,
    db::Database,
    domain::{
        ORGANIZATION_ID,
        audit::{AuditActor, AuditRepository},
        backups::{BackupRepository, CreateBackupTarget},
        groups::GroupRepository,
        machines::MachineRepository,
        projects::ProjectRepository,
        secrets::SecretRepository,
        users::{Role, UserRepository},
    },
};
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use serde_json::json;
use uuid::Uuid;

async fn database() -> (tempfile::TempDir, Database) {
    let data = tempfile::tempdir().expect("tempdir");
    let db = Database::connect(&data.path().join("database-design.sqlite3"))
        .await
        .expect("database");
    (data, db)
}

async fn assert_rejected(db: &Database, sql: impl Into<String>) {
    db.connection()
        .execute_unprepared(&sql.into())
        .await
        .expect_err("database invariant must reject the statement");
}

#[tokio::test]
async fn schema_contains_every_record_table_and_supporting_structure() {
    let (_data, db) = database().await;
    let rows = db
        .connection()
        .query_all_raw(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT name FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name <> 'seaql_migrations' ORDER BY name",
        ))
        .await
        .expect("list schema tables");
    let tables = rows
        .into_iter()
        .map(|row| row.try_get::<String>("", "name").expect("table name"))
        .collect::<Vec<_>>();
    assert_eq!(
        tables,
        [
            "audit_events",
            "audit_settings",
            "backup_jobs",
            "backup_targets",
            "group_members",
            "groups",
            "machine_accounts",
            "machine_group_grants",
            "machine_sessions",
            "machine_user_grants",
            "organizations",
            "project_group_grants",
            "project_machine_grants",
            "project_user_grants",
            "projects",
            "sdk_sync_state",
            "secret_group_grants",
            "secret_machine_grants",
            "secret_user_grants",
            "secrets",
            "sessions",
            "users",
        ]
    );

    let foreign_key_violations = db
        .connection()
        .query_all_raw(Statement::from_string(
            DatabaseBackend::Sqlite,
            "PRAGMA foreign_key_check",
        ))
        .await
        .expect("foreign key check");
    assert!(foreign_key_violations.is_empty());

    let integrity = db
        .connection()
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Sqlite,
            "PRAGMA integrity_check",
        ))
        .await
        .expect("integrity check query")
        .expect("integrity check row")
        .try_get::<String>("", "integrity_check")
        .expect("integrity check result");
    assert_eq!(integrity, "ok");

    let structure_count = db
        .connection()
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Sqlite,
            r#"
            SELECT COUNT(*) AS count
            FROM sqlite_schema
            WHERE name IN (
                'idx_users_role_disabled',
                'idx_secrets_revision',
                'idx_backup_targets_due',
                'idx_backup_jobs_one_running',
                'trg_sdk_sync_revision_monotonic',
                'trg_audit_events_no_update',
                'trg_audit_events_no_delete'
            )
            "#,
        ))
        .await
        .expect("structure query")
        .expect("structure row")
        .try_get::<i64>("", "count")
        .expect("structure count");
    assert_eq!(structure_count, 7);
}

#[tokio::test]
async fn every_table_enforces_its_storage_invariants() {
    let (_data, db) = database().await;
    let user = UserRepository::new(db.clone())
        .create("admin", "Administrator", Role::Admin, "test-password-123")
        .await
        .expect("user");
    let project = ProjectRepository::new(db.clone())
        .create_plain("Production")
        .await
        .expect("project");
    let secret = SecretRepository::new(db.clone())
        .create_plain("DATABASE_URL", "sqlite://data", "runtime", project.id)
        .await
        .expect("secret");
    let machine = MachineRepository::new(db.clone())
        .issue("deploy", user.id)
        .await
        .expect("machine")
        .account;
    let group = GroupRepository::new(db.clone())
        .create("Operators")
        .await
        .expect("group");
    let backup_input: CreateBackupTarget = serde_json::from_value(json!({
        "displayName": "Primary WebDAV",
        "config": { "kind": "WEBDAV", "settings": {
            "endpoint": "https://dav.example.com/storage", "prefix": "daily"
        }},
        "credentials": { "kind": "WEBDAV", "values": {
            "username": "backup-user", "password": "backup-password"
        }},
        "enabled": true,
        "scheduleEnabled": true,
        "intervalHours": 12
    }))
    .expect("backup input");
    let backup_target = BackupRepository::new(db.clone(), MasterKey::random().expect("master key"))
        .create_target(backup_input)
        .await
        .expect("backup target");
    AuditRepository::new(db.clone())
        .record(
            AuditActor::User(user.id),
            "database.test",
            "project",
            Some(project.id),
            "allowed",
        )
        .await
        .expect("audit event");

    assert_rejected(
        &db,
        format!("UPDATE organizations SET name = '   ' WHERE id = '{ORGANIZATION_ID}'"),
    )
    .await;
    assert_rejected(
        &db,
        format!("UPDATE users SET disabled = 2 WHERE id = '{}'", user.id),
    )
    .await;
    assert_rejected(
        &db,
        format!(
            "INSERT INTO sessions (id, user_id, csrf_digest, expires_at, created_at) VALUES ('{}', '{}', '{}', 10, 10)",
            "a".repeat(64),
            user.id,
            "b".repeat(64)
        ),
    )
    .await;
    assert_rejected(
        &db,
        format!(
            "UPDATE projects SET updated_at = created_at - 1 WHERE id = '{}'",
            project.id
        ),
    )
    .await;
    assert_rejected(
        &db,
        format!(
            "UPDATE secrets SET key_plain = NULL WHERE id = '{}'",
            secret.id
        ),
    )
    .await;
    assert_rejected(
        &db,
        format!(
            "UPDATE machine_accounts SET compatibility_account = 2 WHERE id = '{}'",
            machine.id
        ),
    )
    .await;
    assert_rejected(
        &db,
        format!(
            "INSERT INTO machine_sessions (id, machine_account_id, expires_at, created_at) VALUES ('{}', '{}', 10, 10)",
            "c".repeat(64),
            machine.id
        ),
    )
    .await;
    assert_rejected(
        &db,
        format!(
            "UPDATE groups SET updated_at = created_at - 1 WHERE id = '{}'",
            group.id
        ),
    )
    .await;
    assert_rejected(
        &db,
        format!(
            "INSERT INTO group_members (group_id, user_id, created_at) VALUES ('{}', '{}', -1)",
            group.id, user.id
        ),
    )
    .await;

    let invalid_grants = [
        format!(
            "INSERT INTO project_user_grants VALUES ('{}', '{}', 0, 0, 1, 1)",
            project.id, user.id
        ),
        format!(
            "INSERT INTO project_group_grants VALUES ('{}', '{}', 0, 0, 1, 1)",
            project.id, group.id
        ),
        format!(
            "INSERT INTO project_machine_grants VALUES ('{}', '{}', 0, 0, 1, 1)",
            project.id, machine.id
        ),
        format!(
            "INSERT INTO secret_user_grants VALUES ('{}', '{}', 0, 0, 1, 1)",
            secret.id, user.id
        ),
        format!(
            "INSERT INTO secret_group_grants VALUES ('{}', '{}', 0, 0, 1, 1)",
            secret.id, group.id
        ),
        format!(
            "INSERT INTO secret_machine_grants VALUES ('{}', '{}', 0, 0, 1, 1)",
            secret.id, machine.id
        ),
        format!(
            "INSERT INTO machine_user_grants VALUES ('{}', '{}', 0, 0, 1, 1)",
            machine.id, user.id
        ),
        format!(
            "INSERT INTO machine_group_grants VALUES ('{}', '{}', 0, 0, 1, 1)",
            machine.id, group.id
        ),
    ];
    for sql in invalid_grants {
        assert_rejected(&db, sql).await;
    }

    assert_rejected(
        &db,
        format!(
            "UPDATE sdk_sync_state SET revision_nanos = revision_nanos WHERE organization_id = '{ORGANIZATION_ID}'"
        ),
    )
    .await;
    assert_rejected(
        &db,
        format!(
            "INSERT INTO audit_events VALUES ('{}', 'system', '{}', 'database.test', 'project', '{}', 'allowed', 1)",
            Uuid::new_v4(),
            user.id,
            project.id
        ),
    )
    .await;
    assert_rejected(&db, "UPDATE audit_events SET outcome = 'changed'").await;
    assert_rejected(&db, "DELETE FROM audit_events").await;
    assert_rejected(
        &db,
        format!(
            "UPDATE backup_targets SET enabled = 2 WHERE id = '{}'",
            backup_target.id
        ),
    )
    .await;
    assert_rejected(
        &db,
        format!(
            "INSERT INTO backup_jobs (id, target_id, trigger_kind, status, object_key, created_at) VALUES ('{}', '{}', 'manual', 'succeeded', 'lightbws/invalid', 1)",
            Uuid::new_v4(),
            backup_target.id
        ),
    )
    .await;

    let foreign_key_violations = db
        .connection()
        .query_all_raw(Statement::from_string(
            DatabaseBackend::Sqlite,
            "PRAGMA foreign_key_check",
        ))
        .await
        .expect("foreign key check");
    assert!(foreign_key_violations.is_empty());
}
