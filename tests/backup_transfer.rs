use lightbws::{
    config::BootstrapAdmin,
    crypto::MasterKey,
    db::{
        Database,
        entities::{
            backup_job, backup_target, project as project_entity, project_group_grant,
            user as user_entity,
        },
    },
    domain::{
        access::{AccessPolicyInput, AccessRepository, GrantInput},
        audit::{AuditActor, AuditRepository},
        backups::{BackupRepository, CreateBackupTarget, recover_interrupted_jobs},
        groups::GroupRepository,
        machines::MachineRepository,
        projects::ProjectRepository,
        secrets::SecretRepository,
        transfer::{
            ArchiveKind, BackupScopes, decrypt_backup, decrypt_portable, dump_database,
            dump_database_scoped, encode_plain_backup, encrypt_backup, encrypt_portable,
            import_database, import_database_scoped, inspect_archive,
        },
        users::{Role, UserRepository},
    },
};
use secrecy::SecretString;

#[test]
fn backup_scope_defaults_and_dependencies_are_explicit() {
    let default = BackupScopes::default();
    assert!(!default.identities);
    assert!(!default.machine_accounts);
    assert!(!default.access_policies);
    assert!(!default.audit);
    assert!(!default.backup_targets);
    default.validate().expect("default scopes");

    let full = BackupScopes::full_instance();
    assert!(full.identities);
    assert!(full.machine_accounts);
    assert!(full.access_policies);
    assert!(full.audit);
    assert!(full.backup_targets);
    full.validate().expect("full scopes");

    let invalid = BackupScopes {
        access_policies: true,
        ..BackupScopes::default()
    };
    assert!(invalid.validate().is_err());
}

#[test]
fn automatic_and_plain_archives_are_detected_and_round_trip() {
    let plaintext = br#"{"version":2,"projects":[],"secrets":[]}"#;
    let master_key = MasterKey::random().expect("master key");
    let encrypted = encrypt_backup(&master_key, plaintext).expect("encrypt backup");
    assert_eq!(
        inspect_archive(&encrypted).expect("inspect"),
        ArchiveKind::MasterKey
    );
    assert_eq!(
        decrypt_backup(&master_key, &encrypted).expect("decrypt backup"),
        plaintext
    );

    let plain = encode_plain_backup(plaintext).expect("plain backup");
    assert_eq!(
        inspect_archive(&plain).expect("inspect"),
        ArchiveKind::Plaintext
    );
    assert_eq!(
        lightbws::domain::transfer::decode_plain_backup(&plain).expect("decode plain"),
        plaintext
    );
}
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait, PaginatorTrait};
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
        .create_plain("DATABASE_URL", "sqlite://production", "runtime", project.id)
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
    let secret = SecretRepository::new(target.clone())
        .list(false, None)
        .await
        .expect("list secrets")
        .pop()
        .expect("imported secret");
    assert_eq!(secret.value_plain.as_deref(), Some("sqlite://production"));
    assert!(
        import_database_scoped(
            &target,
            &MasterKey::random().expect("legacy key"),
            &plaintext,
            true,
            false,
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn full_instance_dump_rebuilds_persistent_application_state() {
    let source_data = tempfile::tempdir().expect("source tempdir");
    let source = Database::connect(&source_data.path().join("source-full.sqlite3"))
        .await
        .expect("source database");
    let users = UserRepository::new(source.clone());
    users
        .bootstrap(Some(&BootstrapAdmin {
            username: "admin".into(),
            password: SecretString::from("correct-horse-battery-staple"),
        }))
        .await
        .expect("bootstrap admin");
    let admin = users.list().await.expect("users").remove(0);
    let member = users
        .create("member", "Member", Role::User, "another-long-password")
        .await
        .expect("member");
    let groups = GroupRepository::new(source.clone());
    let group = groups.create("Operators").await.expect("group");
    groups
        .replace_members(group.id, &[member.id])
        .await
        .expect("membership");
    let machine = MachineRepository::new(source.clone())
        .issue("Deployment", admin.id)
        .await
        .expect("machine");
    ProjectRepository::new(source.clone())
        .create_plain("Production")
        .await
        .expect("project");
    let project = ProjectRepository::new(source.clone())
        .list(false)
        .await
        .expect("projects")
        .remove(0);
    SecretRepository::new(source.clone())
        .create_plain(
            "DATABASE_URL",
            "sqlite://production",
            "runtime",
            Uuid::parse_str(&project.id).expect("project id"),
        )
        .await
        .expect("secret");
    AccessRepository::new(source.clone())
        .replace_project(
            Uuid::parse_str(&project.id).expect("project id"),
            &AccessPolicyInput {
                users: vec![],
                groups: vec![GrantInput {
                    grantee_id: group.id,
                    read: true,
                    write: false,
                }],
                machines: vec![GrantInput {
                    grantee_id: machine.account.id,
                    read: true,
                    write: false,
                }],
            },
        )
        .await
        .expect("access policy");
    let source_key = MasterKey::random().expect("source key");
    BackupRepository::new(source.clone(), source_key.clone())
        .with_plaintext_allowed(true)
        .create_target(
            serde_json::from_value(json!({
                "displayName": "Primary WebDAV",
                "config": { "kind": "WEBDAV", "settings": {
                    "endpoint": "https://dav.example.com/storage", "prefix": "daily"
                }},
                "credentials": { "kind": "WEBDAV", "values": {
                    "username": "backup-user", "password": "password-sentinel"
                }},
                "enabled": true,
                "scheduleEnabled": false,
                "intervalHours": 12,
                "encryption": "plaintext",
                "confirmPlaintext": true
            }))
            .expect("target input"),
        )
        .await
        .expect("target");
    AuditRepository::new(source.clone())
        .record(
            AuditActor::User(admin.id),
            "backup.test",
            "backup",
            None,
            "allowed",
        )
        .await
        .expect("audit");

    let dump = dump_database_scoped(&source, &source_key, BackupScopes::full_instance())
        .await
        .expect("full dump");
    let target_data = tempfile::tempdir().expect("target tempdir");
    let target = Database::connect(&target_data.path().join("target-full.sqlite3"))
        .await
        .expect("target database");
    let target_key = MasterKey::random().expect("target key");
    let summary = import_database_scoped(&target, &target_key, &dump, true, false)
        .await
        .expect("full import");
    assert!(summary.full_instance);
    import_database_scoped(&target, &target_key, &dump, false, false)
        .await
        .expect("idempotent full merge");
    assert_eq!(
        UserRepository::new(target.clone())
            .list()
            .await
            .expect("users")
            .len(),
        2
    );
    assert_eq!(
        GroupRepository::new(target.clone())
            .list()
            .await
            .expect("groups")
            .len(),
        1
    );
    assert_eq!(
        MachineRepository::new(target.clone())
            .list()
            .await
            .expect("machines")
            .len(),
        1
    );
    assert_eq!(
        ProjectRepository::new(target.clone())
            .list(false)
            .await
            .expect("projects")
            .len(),
        1
    );
    assert_eq!(
        SecretRepository::new(target.clone())
            .list(false, None)
            .await
            .expect("secrets")
            .len(),
        1
    );
    assert_eq!(
        project_group_grant::Entity::find()
            .count(target.connection())
            .await
            .expect("project group grants"),
        1
    );
    assert_eq!(
        AuditRepository::new(target.clone())
            .list()
            .await
            .expect("audit")
            .len(),
        1
    );
    let restored_target = BackupRepository::new(target, target_key)
        .list_targets()
        .await
        .expect("targets")
        .remove(0);
    assert!(!restored_target.enabled);
    assert!(!restored_target.schedule_enabled);
    assert_eq!(
        restored_target.encryption,
        lightbws::domain::backups::BackupEncryption::MasterKey
    );
}

#[tokio::test]
async fn scoped_import_merges_by_id_and_rolls_back_unique_conflicts() {
    let source_data = tempfile::tempdir().expect("source tempdir");
    let source = Database::connect(&source_data.path().join("source-merge.sqlite3"))
        .await
        .expect("source database");
    let users = UserRepository::new(source.clone());
    users
        .create("first", "First", Role::Admin, "test-password-123")
        .await
        .expect("first user");
    users
        .create("second", "Second", Role::User, "test-password-123")
        .await
        .expect("second user");
    ProjectRepository::new(source.clone())
        .create_plain("Merge me")
        .await
        .expect("project");
    let key = MasterKey::random().expect("master key");
    let scopes = BackupScopes {
        identities: true,
        ..BackupScopes::default()
    };
    let dump = dump_database_scoped(&source, &key, scopes)
        .await
        .expect("scoped dump");

    let target_data = tempfile::tempdir().expect("target tempdir");
    let target = Database::connect(&target_data.path().join("target-merge.sqlite3"))
        .await
        .expect("target database");
    import_database_scoped(&target, &key, &dump, false, false)
        .await
        .expect("first import");
    import_database_scoped(&target, &key, &dump, false, false)
        .await
        .expect("merge same identifiers");
    assert_eq!(
        user_entity::Entity::find()
            .count(target.connection())
            .await
            .expect("users"),
        2
    );

    let rollback_data = tempfile::tempdir().expect("rollback tempdir");
    let rollback = Database::connect(&rollback_data.path().join("rollback.sqlite3"))
        .await
        .expect("rollback database");
    let mut forged: serde_json::Value = serde_json::from_slice(&dump).expect("dump json");
    let users = forged["users"].as_array_mut().expect("users array");
    users[1]["username"] = users[0]["username"].clone();
    let forged = serde_json::to_vec(&forged).expect("forged dump");
    assert!(
        import_database_scoped(&rollback, &key, &forged, false, false)
            .await
            .is_err()
    );
    assert_eq!(
        user_entity::Entity::find()
            .count(rollback.connection())
            .await
            .expect("rolled back users"),
        0
    );
    assert_eq!(
        project_entity::Entity::find()
            .count(rollback.connection())
            .await
            .expect("rolled back projects"),
        0
    );
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

    let plaintext_input: CreateBackupTarget = serde_json::from_value(json!({
        "displayName": "Unsafe WebDAV",
        "config": { "kind": "WEBDAV", "settings": {
            "endpoint": "https://dav.example.com/storage", "prefix": "plain"
        }},
        "credentials": { "kind": "WEBDAV", "values": {
            "username": "backup-user", "password": "password-sentinel"
        }},
        "enabled": true,
        "scheduleEnabled": false,
        "intervalHours": 12,
        "encryption": "plaintext",
        "confirmPlaintext": true
    }))
    .expect("plaintext input");
    assert!(
        repository
            .create_target(plaintext_input.clone())
            .await
            .is_err()
    );
    let mut unconfirmed = plaintext_input.clone();
    unconfirmed.confirm_plaintext = false;
    assert!(
        BackupRepository::new(db.clone(), MasterKey::random().expect("confirmation key"))
            .with_plaintext_allowed(true)
            .create_target(unconfirmed)
            .await
            .is_err()
    );
    BackupRepository::new(db.clone(), MasterKey::random().expect("second key"))
        .with_plaintext_allowed(true)
        .create_target(plaintext_input)
        .await
        .expect("explicitly allowed plaintext target");

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
