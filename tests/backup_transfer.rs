use lightbws::{
    crypto::MasterKey,
    db::{
        Database,
        entities::{backup_job, backup_target},
    },
    domain::{
        backups::{BackupRepository, CreateBackupTarget, recover_interrupted_jobs},
        projects::ProjectRepository,
        secrets::SecretRepository,
        transfer::{decrypt_portable, dump_database, encrypt_portable, import_database},
    },
};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait};
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn portable_export_round_trips_projects_and_secrets() {
    let source_data = tempfile::tempdir().expect("source tempdir");
    let source = Database::connect(&source_data.path().join("source.sqlite3"))
        .await
        .expect("source database");
    let project = ProjectRepository::new(source.clone())
        .create_plain("Production")
        .await
        .expect("project");
    SecretRepository::new(source.clone())
        .create_plain(
            "DATABASE_URL",
            "sqlite://production",
            "runtime",
            Some(project.id),
        )
        .await
        .expect("secret");

    let envelope = encrypt_portable(
        "portable-passphrase-123",
        &dump_database(&source).await.expect("dump"),
    )
    .expect("encrypt");
    assert!(!String::from_utf8_lossy(&envelope).contains("sqlite://production"));
    let plaintext = decrypt_portable("portable-passphrase-123", &envelope).expect("decrypt");

    let target_data = tempfile::tempdir().expect("target tempdir");
    let target = Database::connect(&target_data.path().join("target.sqlite3"))
        .await
        .expect("target database");
    let summary = import_database(&target, &plaintext).await.expect("import");
    assert_eq!(summary.projects, 1);
    assert_eq!(summary.secrets, 1);
    let secret = SecretRepository::new(target)
        .list(false, None)
        .await
        .expect("list secrets")
        .pop()
        .expect("imported secret");
    assert_eq!(secret.value_plain.as_deref(), Some("sqlite://production"));
}

#[tokio::test]
async fn backup_credentials_are_encrypted_and_never_returned() {
    let data = tempfile::tempdir().expect("tempdir");
    let db = Database::connect(&data.path().join("backup.sqlite3"))
        .await
        .expect("database");
    let repository = BackupRepository::new(db.clone(), MasterKey::random().expect("master key"));
    let input: CreateBackupTarget = serde_json::from_value(json!({
        "displayName": "Primary WebDAV",
        "config": { "kind": "WEBDAV", "settings": {
            "endpoint": "https://dav.example.com/storage", "prefix": "daily"
        }},
        "credentials": { "kind": "WEBDAV", "values": {
            "username": "backup-user", "password": "password-sentinel"
        }},
        "enabled": true,
        "scheduleEnabled": true,
        "intervalHours": 12
    }))
    .expect("backup input");
    let target = repository
        .create_target(input)
        .await
        .expect("create target");
    let serialized = serde_json::to_string(&target).expect("serialize target");
    assert!(target.has_credentials);
    assert!(!serialized.contains("password-sentinel"));
    assert!(!serialized.contains("backup-user"));

    let stored = backup_target::Entity::find_by_id(target.id.to_string())
        .one(db.connection())
        .await
        .expect("query")
        .expect("stored target");
    assert!(!stored.credentials_cipher.contains("password-sentinel"));
    assert!(!stored.credentials_cipher.contains("backup-user"));
    assert!(!stored.public_config_json.contains("password"));

    let running_id = Uuid::new_v4();
    backup_job::ActiveModel {
        id: Set(running_id.to_string()),
        target_id: Set(target.id.to_string()),
        trigger_kind: Set("manual".into()),
        status: Set("running".into()),
        object_key: Set("lightbws/test-one.lightbws".into()),
        byte_size: Set(None),
        error_code: Set(None),
        created_at: Set(1),
        completed_at: Set(None),
    }
    .insert(db.connection())
    .await
    .expect("first running job");
    let duplicate = backup_job::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        target_id: Set(target.id.to_string()),
        trigger_kind: Set("scheduled".into()),
        status: Set("running".into()),
        object_key: Set("lightbws/test-two.lightbws".into()),
        byte_size: Set(None),
        error_code: Set(None),
        created_at: Set(2),
        completed_at: Set(None),
    }
    .insert(db.connection())
    .await;
    assert!(duplicate.is_err());

    recover_interrupted_jobs(&db)
        .await
        .expect("recover interrupted jobs");
    let recovered = backup_job::Entity::find_by_id(running_id.to_string())
        .one(db.connection())
        .await
        .expect("query recovered job")
        .expect("recovered job");
    assert_eq!(recovered.status, "failed");
    assert_eq!(recovered.error_code.as_deref(), Some("interrupted"));
}
