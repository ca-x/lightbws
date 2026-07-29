use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, Payload},
};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::TryRng;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseBackend, DbErr,
    EntityTrait, IntoActiveModel, QueryFilter, Statement, TransactionTrait,
};
use serde::{Deserialize, Serialize};

use crate::{
    crypto::MasterKey,
    db::{
        Database,
        entities::{
            audit_event, audit_setting, backup_target, group, group_member, machine_account,
            machine_group_grant, machine_user_grant, project, project_group_grant,
            project_machine_grant, project_user_grant, secret, secret_group_grant,
            secret_machine_grant, secret_user_grant, user,
        },
    },
    domain::{
        ORGANIZATION_ID,
        backups::{
            BackupCredentials, BackupEncryption, BackupPublicConfig, decode_credentials,
            encode_credentials, validate_restored_target,
        },
        next_sdk_revision,
    },
    error::AppError,
};

const EXPORT_MAGIC: &[u8; 8] = b"LBWSX01\0";
const AUTOMATIC_MAGIC: &[u8] = b"LIGHTBWS-BACKUP-V1\n";
const PLAIN_MAGIC: &[u8] = b"LIGHTBWS-PLAIN-V2\n";
const BACKUP_AAD: &[u8] = b"lightbws-backup-v1";
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const MAX_EXPORT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackupScopes {
    #[serde(default)]
    pub identities: bool,
    #[serde(default)]
    pub machine_accounts: bool,
    #[serde(default)]
    pub access_policies: bool,
    #[serde(default)]
    pub audit: bool,
    #[serde(default)]
    pub backup_targets: bool,
}

impl BackupScopes {
    pub const fn full_instance() -> Self {
        Self {
            identities: true,
            machine_accounts: true,
            access_policies: true,
            audit: true,
            backup_targets: true,
        }
    }

    pub fn validate(self) -> Result<(), AppError> {
        if self.machine_accounts && !self.identities {
            return Err(AppError::Validation(
                "backup scope dependencies are incomplete".into(),
            ));
        }
        if self.access_policies && (!self.identities || !self.machine_accounts) {
            return Err(AppError::Validation(
                "backup scope dependencies are incomplete".into(),
            ));
        }
        Ok(())
    }

    pub const fn is_full_instance(self) -> bool {
        self.identities
            && self.machine_accounts
            && self.access_policies
            && self.audit
            && self.backup_targets
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ArchiveKind {
    Passphrase,
    MasterKey,
    Plaintext,
}

pub fn inspect_archive(value: &[u8]) -> Result<ArchiveKind, AppError> {
    if value.starts_with(EXPORT_MAGIC) {
        Ok(ArchiveKind::Passphrase)
    } else if value.starts_with(AUTOMATIC_MAGIC) {
        Ok(ArchiveKind::MasterKey)
    } else if value.starts_with(PLAIN_MAGIC) {
        Ok(ArchiveKind::Plaintext)
    } else {
        Err(AppError::Validation("invalid LightBWS backup".into()))
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatabaseDump {
    version: u8,
    exported_at: i64,
    projects: Vec<ProjectRecord>,
    secrets: Vec<SecretRecord>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DatabaseDumpV2 {
    version: u8,
    exported_at: i64,
    scopes: BackupScopes,
    projects: Vec<ProjectRecord>,
    secrets: Vec<SecretRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    users: Option<Vec<user::Model>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    groups: Option<Vec<group::Model>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    group_members: Option<Vec<group_member::Model>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    machine_accounts: Option<Vec<machine_account::Model>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    project_user_grants: Option<Vec<project_user_grant::Model>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    project_group_grants: Option<Vec<project_group_grant::Model>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    project_machine_grants: Option<Vec<project_machine_grant::Model>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    secret_user_grants: Option<Vec<secret_user_grant::Model>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    secret_group_grants: Option<Vec<secret_group_grant::Model>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    secret_machine_grants: Option<Vec<secret_machine_grant::Model>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    machine_user_grants: Option<Vec<machine_user_grant::Model>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    machine_group_grants: Option<Vec<machine_group_grant::Model>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    audit_settings: Option<Vec<audit_setting::Model>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    audit_events: Option<Vec<audit_event::Model>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    backup_targets: Option<Vec<BackupTargetRecord>>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BackupTargetRecord {
    id: String,
    display_name: String,
    config: BackupPublicConfig,
    credentials: BackupCredentials,
    scopes: BackupScopes,
    encryption: BackupEncryption,
    enabled: bool,
    schedule_enabled: bool,
    interval_hours: i32,
    next_run_at: Option<i64>,
    last_run_at: Option<i64>,
    last_status: Option<String>,
    last_error: Option<String>,
    created_at: i64,
    updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectRecord {
    id: String,
    organization_id: String,
    name_cipher: Option<String>,
    name_plain: Option<String>,
    deleted_at: Option<i64>,
    created_at: i64,
    updated_at: i64,
    revision_nanos: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SecretRecord {
    id: String,
    organization_id: String,
    project_id: String,
    key_cipher: Option<String>,
    value_cipher: Option<String>,
    note_cipher: Option<String>,
    key_plain: Option<String>,
    value_plain: Option<String>,
    note_plain: Option<String>,
    deleted_at: Option<i64>,
    created_at: i64,
    updated_at: i64,
    revision_nanos: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub projects: usize,
    pub secrets: usize,
    pub full_instance: bool,
}

pub async fn dump_database(db: &Database) -> Result<Vec<u8>, AppError> {
    let transaction = db.connection().begin().await?;
    let estimate = transaction
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Sqlite,
            r#"
            SELECT
                COALESCE((SELECT SUM(
                    LENGTH(id) + LENGTH(organization_id) + COALESCE(LENGTH(name_cipher), 0)
                    + COALESCE(LENGTH(name_plain), 0) + 512
                ) FROM projects), 0)
                + COALESCE((SELECT SUM(
                    LENGTH(id) + LENGTH(project_id)
                    + COALESCE(LENGTH(key_cipher), 0) + COALESCE(LENGTH(value_cipher), 0)
                    + COALESCE(LENGTH(note_cipher), 0) + COALESCE(LENGTH(key_plain), 0)
                    + COALESCE(LENGTH(value_plain), 0) + COALESCE(LENGTH(note_plain), 0) + 768
                ) FROM secrets), 0) AS estimated_bytes
            "#,
        ))
        .await?
        .ok_or_else(|| AppError::internal(anyhow::anyhow!("export size query failed")))?
        .try_get::<i64>("", "estimated_bytes")?;
    if estimate > i64::try_from(MAX_EXPORT_BYTES).expect("export limit fits in i64") {
        return Err(AppError::Validation(
            "database export exceeds the 64 MiB safety limit".into(),
        ));
    }
    let projects = project::Entity::find()
        .all(&transaction)
        .await?
        .into_iter()
        .map(ProjectRecord::from)
        .collect();
    let secrets = secret::Entity::find()
        .all(&transaction)
        .await?
        .into_iter()
        .map(SecretRecord::from)
        .collect();
    transaction.commit().await?;
    let serialized = serde_json::to_vec(&DatabaseDump {
        version: 1,
        exported_at: super::now(),
        projects,
        secrets,
    })
    .map_err(AppError::internal)?;
    if serialized.len() > MAX_EXPORT_BYTES {
        return Err(AppError::Validation(
            "database export exceeds the 64 MiB safety limit".into(),
        ));
    }
    Ok(serialized)
}

pub async fn dump_database_scoped(
    db: &Database,
    master_key: &MasterKey,
    scopes: BackupScopes,
) -> Result<Vec<u8>, AppError> {
    scopes.validate()?;
    let transaction = db.connection().begin().await?;
    let projects = project::Entity::find()
        .all(&transaction)
        .await?
        .into_iter()
        .map(ProjectRecord::from)
        .collect();
    let secrets = secret::Entity::find()
        .all(&transaction)
        .await?
        .into_iter()
        .map(SecretRecord::from)
        .collect();
    let users = optional_models::<user::Entity>(&transaction, scopes.identities).await?;
    let groups = optional_models::<group::Entity>(&transaction, scopes.identities).await?;
    let group_members =
        optional_models::<group_member::Entity>(&transaction, scopes.identities).await?;
    let machine_accounts =
        optional_models::<machine_account::Entity>(&transaction, scopes.machine_accounts).await?;
    let project_user_grants =
        optional_models::<project_user_grant::Entity>(&transaction, scopes.access_policies).await?;
    let project_group_grants =
        optional_models::<project_group_grant::Entity>(&transaction, scopes.access_policies)
            .await?;
    let project_machine_grants =
        optional_models::<project_machine_grant::Entity>(&transaction, scopes.access_policies)
            .await?;
    let secret_user_grants =
        optional_models::<secret_user_grant::Entity>(&transaction, scopes.access_policies).await?;
    let secret_group_grants =
        optional_models::<secret_group_grant::Entity>(&transaction, scopes.access_policies).await?;
    let secret_machine_grants =
        optional_models::<secret_machine_grant::Entity>(&transaction, scopes.access_policies)
            .await?;
    let machine_user_grants =
        optional_models::<machine_user_grant::Entity>(&transaction, scopes.access_policies).await?;
    let machine_group_grants =
        optional_models::<machine_group_grant::Entity>(&transaction, scopes.access_policies)
            .await?;
    let audit_settings =
        optional_models::<audit_setting::Entity>(&transaction, scopes.audit).await?;
    let audit_events = optional_models::<audit_event::Entity>(&transaction, scopes.audit).await?;
    let backup_targets = if scopes.backup_targets {
        let mut records = Vec::new();
        for model in backup_target::Entity::find().all(&transaction).await? {
            let id = uuid::Uuid::parse_str(&model.id).map_err(AppError::internal)?;
            let plaintext = master_key
                .decrypt(id.as_bytes(), &model.credentials_cipher)
                .map_err(AppError::internal)?;
            records.push(BackupTargetRecord {
                id: model.id,
                display_name: model.display_name,
                config: serde_json::from_str(&model.public_config_json)
                    .map_err(AppError::internal)?,
                credentials: decode_credentials(&model.kind, &plaintext)?,
                scopes: serde_json::from_str(&model.scopes_json).map_err(AppError::internal)?,
                encryption: match model.encryption_mode.as_str() {
                    "master_key" => BackupEncryption::MasterKey,
                    "plaintext" => BackupEncryption::Plaintext,
                    _ => {
                        return Err(AppError::internal(anyhow::anyhow!(
                            "invalid stored backup encryption mode"
                        )));
                    }
                },
                enabled: model.enabled,
                schedule_enabled: model.schedule_enabled,
                interval_hours: model.interval_hours,
                next_run_at: model.next_run_at,
                last_run_at: model.last_run_at,
                last_status: model.last_status,
                last_error: model.last_error,
                created_at: model.created_at,
                updated_at: model.updated_at,
            });
        }
        Some(records)
    } else {
        None
    };
    transaction.commit().await?;
    let serialized = serde_json::to_vec(&DatabaseDumpV2 {
        version: 2,
        exported_at: super::now(),
        scopes,
        projects,
        secrets,
        users,
        groups,
        group_members,
        machine_accounts,
        project_user_grants,
        project_group_grants,
        project_machine_grants,
        secret_user_grants,
        secret_group_grants,
        secret_machine_grants,
        machine_user_grants,
        machine_group_grants,
        audit_settings,
        audit_events,
        backup_targets,
    })
    .map_err(AppError::internal)?;
    if serialized.len() > MAX_EXPORT_BYTES {
        return Err(AppError::Validation(
            "database export exceeds the 64 MiB safety limit".into(),
        ));
    }
    Ok(serialized)
}

async fn optional_models<E>(
    connection: &impl ConnectionTrait,
    selected: bool,
) -> Result<Option<Vec<E::Model>>, AppError>
where
    E: EntityTrait,
    E::Model: Send + Sync,
{
    if selected {
        Ok(Some(E::find().all(connection).await?))
    } else {
        Ok(None)
    }
}

pub async fn import_database(db: &Database, bytes: &[u8]) -> Result<ImportSummary, AppError> {
    validate_import_size(bytes)?;
    let dump: DatabaseDump = serde_json::from_slice(bytes)
        .map_err(|_| AppError::Validation("invalid LightBWS export".into()))?;
    if dump.version != 1
        || dump
            .projects
            .iter()
            .any(|record| record.organization_id != ORGANIZATION_ID)
        || dump
            .secrets
            .iter()
            .any(|record| record.organization_id != ORGANIZATION_ID)
    {
        return Err(AppError::Validation("unsupported LightBWS export".into()));
    }
    let project_count = dump.projects.len();
    let secret_count = dump.secrets.len();
    let transaction = db.connection().begin().await?;
    let import_revision = if secret_count == 0 {
        None
    } else {
        Some(next_sdk_revision(&transaction).await?)
    };
    for record in dump.projects {
        validate_project_record(&record)?;
        if let Some(model) = project::Entity::find_by_id(&record.id)
            .one(&transaction)
            .await?
        {
            let mut active = model.into_active_model();
            active.organization_id = Set(record.organization_id);
            active.name_cipher = Set(record.name_cipher);
            active.name_plain = Set(record.name_plain);
            active.deleted_at = Set(record.deleted_at);
            active.created_at = Set(record.created_at);
            active.updated_at = Set(record.updated_at);
            active.revision_nanos = Set(record.revision_nanos);
            active.update(&transaction).await?;
        } else {
            project::ActiveModel {
                id: Set(record.id),
                organization_id: Set(record.organization_id),
                name_cipher: Set(record.name_cipher),
                name_plain: Set(record.name_plain),
                deleted_at: Set(record.deleted_at),
                created_at: Set(record.created_at),
                updated_at: Set(record.updated_at),
                revision_nanos: Set(record.revision_nanos),
            }
            .insert(&transaction)
            .await?;
        }
    }
    for record in dump.secrets {
        validate_secret_record(&record)?;
        if let Some(model) = secret::Entity::find_by_id(&record.id)
            .one(&transaction)
            .await?
        {
            let mut active = model.into_active_model();
            apply_secret(&mut active, record, import_revision);
            active.update(&transaction).await?;
        } else {
            let mut active = secret::ActiveModel {
                id: Set(record.id.clone()),
                ..Default::default()
            };
            apply_secret(&mut active, record, import_revision);
            active.insert(&transaction).await?;
        }
    }
    transaction.commit().await?;
    Ok(ImportSummary {
        projects: project_count,
        secrets: secret_count,
        full_instance: false,
    })
}

pub async fn import_database_scoped(
    db: &Database,
    master_key: &MasterKey,
    bytes: &[u8],
    replace: bool,
    allow_plaintext_backups: bool,
) -> Result<ImportSummary, AppError> {
    validate_import_size(bytes)?;
    let version = serde_json::from_slice::<serde_json::Value>(bytes)
        .ok()
        .and_then(|value| value.get("version").and_then(serde_json::Value::as_u64));
    if version == Some(1) {
        if replace {
            return Err(AppError::Validation(
                "replace restore requires a version 2 full-instance backup".into(),
            ));
        }
        return import_database(db, bytes).await;
    }
    let dump: DatabaseDumpV2 = serde_json::from_slice(bytes)
        .map_err(|_| AppError::Validation("invalid LightBWS export".into()))?;
    if dump.version != 2 {
        return Err(AppError::Validation("unsupported LightBWS export".into()));
    }
    let scopes = dump.scopes;
    scopes.validate()?;
    validate_v2_sections(&dump)?;
    if scopes.audit && !matches!(dump.audit_settings.as_deref(), Some([setting]) if setting.id == 1)
    {
        return Err(AppError::Validation(
            "backup must contain the singleton audit setting".into(),
        ));
    }
    if replace && !dump.scopes.is_full_instance() {
        return Err(AppError::Validation(
            "replace restore requires a full-instance backup".into(),
        ));
    }
    if dump
        .projects
        .iter()
        .any(|record| record.organization_id != ORGANIZATION_ID)
        || dump
            .secrets
            .iter()
            .any(|record| record.organization_id != ORGANIZATION_ID)
    {
        return Err(AppError::Validation("unsupported LightBWS export".into()));
    }
    for record in &dump.projects {
        validate_project_record(record)?;
    }
    for record in &dump.secrets {
        validate_secret_record(record)?;
    }
    let project_count = dump.projects.len();
    let secret_count = dump.secrets.len();
    let transaction = db.connection().begin().await?;
    if replace {
        clear_full_instance(&transaction).await?;
    }
    if scopes.identities {
        connection_delete(&transaction, "DELETE FROM sessions").await?;
    }
    if scopes.machine_accounts {
        connection_delete(&transaction, "DELETE FROM machine_sessions").await?;
    }

    upsert_models::<user::ActiveModel, _>(&transaction, dump.users.unwrap_or_default()).await?;
    for record in dump.projects {
        upsert_model(
            &transaction,
            project::ActiveModel {
                id: Set(record.id),
                organization_id: Set(record.organization_id),
                name_cipher: Set(record.name_cipher),
                name_plain: Set(record.name_plain),
                deleted_at: Set(record.deleted_at),
                created_at: Set(record.created_at),
                updated_at: Set(record.updated_at),
                revision_nanos: Set(record.revision_nanos),
            },
        )
        .await?;
    }
    for record in dump.secrets {
        let mut active = secret::ActiveModel {
            id: Set(record.id.clone()),
            ..Default::default()
        };
        apply_secret(&mut active, record, None);
        upsert_model(&transaction, active).await?;
    }
    upsert_models::<group::ActiveModel, _>(&transaction, dump.groups.unwrap_or_default()).await?;
    upsert_models::<group_member::ActiveModel, _>(
        &transaction,
        dump.group_members.unwrap_or_default(),
    )
    .await?;
    upsert_models::<machine_account::ActiveModel, _>(
        &transaction,
        dump.machine_accounts.unwrap_or_default(),
    )
    .await?;
    upsert_models::<project_user_grant::ActiveModel, _>(
        &transaction,
        dump.project_user_grants.unwrap_or_default(),
    )
    .await?;
    upsert_models::<project_group_grant::ActiveModel, _>(
        &transaction,
        dump.project_group_grants.unwrap_or_default(),
    )
    .await?;
    upsert_models::<project_machine_grant::ActiveModel, _>(
        &transaction,
        dump.project_machine_grants.unwrap_or_default(),
    )
    .await?;
    upsert_models::<secret_user_grant::ActiveModel, _>(
        &transaction,
        dump.secret_user_grants.unwrap_or_default(),
    )
    .await?;
    upsert_models::<secret_group_grant::ActiveModel, _>(
        &transaction,
        dump.secret_group_grants.unwrap_or_default(),
    )
    .await?;
    upsert_models::<secret_machine_grant::ActiveModel, _>(
        &transaction,
        dump.secret_machine_grants.unwrap_or_default(),
    )
    .await?;
    upsert_models::<machine_user_grant::ActiveModel, _>(
        &transaction,
        dump.machine_user_grants.unwrap_or_default(),
    )
    .await?;
    upsert_models::<machine_group_grant::ActiveModel, _>(
        &transaction,
        dump.machine_group_grants.unwrap_or_default(),
    )
    .await?;
    let audit_settings = dump
        .audit_settings
        .unwrap_or_default()
        .into_iter()
        .map(|mut setting| {
            setting.cleanup_authorized = false;
            setting
        })
        .collect();
    upsert_models::<audit_setting::ActiveModel, _>(&transaction, audit_settings).await?;
    merge_audit_events(&transaction, dump.audit_events.unwrap_or_default()).await?;
    for record in dump.backup_targets.unwrap_or_default() {
        let id = uuid::Uuid::parse_str(&record.id)
            .map_err(|_| AppError::Validation("invalid backup target identifier".into()))?;
        let encryption =
            if record.encryption == BackupEncryption::Plaintext && !allow_plaintext_backups {
                BackupEncryption::MasterKey
            } else {
                record.encryption
            };
        let (display_name, config, interval_hours) = validate_restored_target(
            &record.display_name,
            record.config,
            &record.credentials,
            record.interval_hours,
            record.scopes,
            encryption,
            allow_plaintext_backups,
        )?;
        let credentials = encode_credentials(&record.credentials)?;
        upsert_model(
            &transaction,
            backup_target::ActiveModel {
            id: Set(record.id),
            display_name: Set(display_name),
            kind: Set(config.kind().into()),
            public_config_json: Set(
                serde_json::to_string(&config).map_err(AppError::internal)?
            ),
            credentials_cipher: Set(master_key
                .encrypt(id.as_bytes(), &credentials)
                .map_err(AppError::internal)?),
            scopes_json: Set(serde_json::to_string(&record.scopes).map_err(AppError::internal)?),
                encryption_mode: Set(match encryption {
                BackupEncryption::MasterKey => "master_key",
                BackupEncryption::Plaintext => "plaintext",
            }
            .into()),
            // Imported destinations may be attacker-controlled. Require an
            // administrator to review and explicitly re-enable every target.
            enabled: Set(false),
            schedule_enabled: Set(false),
            interval_hours: Set(i32::from(interval_hours)),
            next_run_at: Set(None),
            last_run_at: Set(record.last_run_at),
            last_status: Set(record.last_status),
            last_error: Set(record.last_error),
            created_at: Set(record.created_at),
            updated_at: Set(record.updated_at),
            },
        )
        .await?;
    }
    if secret_count > 0 {
        next_sdk_revision(&transaction).await?;
    }
    if scopes.identities
        && user::Entity::find()
            .filter(user::Column::Role.eq("admin"))
            .filter(user::Column::Disabled.eq(false))
            .one(&transaction)
            .await?
            .is_none()
    {
        return Err(AppError::Validation(
            "backup restore must leave at least one active administrator".into(),
        ));
    }
    transaction.commit().await?;
    Ok(ImportSummary {
        projects: project_count,
        secrets: secret_count,
        full_instance: scopes.is_full_instance(),
    })
}

async fn connection_delete(
    connection: &impl ConnectionTrait,
    statement: &str,
) -> Result<(), AppError> {
    connection.execute_unprepared(statement).await?;
    Ok(())
}

async fn merge_audit_events(
    connection: &impl ConnectionTrait,
    events: Vec<audit_event::Model>,
) -> Result<(), AppError> {
    for event in events {
        match audit_event::Entity::find_by_id(&event.id)
            .one(connection)
            .await?
        {
            Some(existing) if existing == event => {}
            Some(_) => {
                return Err(AppError::Validation(
                    "backup conflicts with an existing immutable audit event".into(),
                ));
            }
            None => {
                event.into_active_model().insert(connection).await?;
            }
        }
    }
    Ok(())
}

fn validate_import_size(bytes: &[u8]) -> Result<(), AppError> {
    if bytes.is_empty() || bytes.len() > MAX_EXPORT_BYTES {
        return Err(AppError::Validation(
            "database import exceeds the 64 MiB safety limit".into(),
        ));
    }
    Ok(())
}

fn validate_v2_sections(dump: &DatabaseDumpV2) -> Result<(), AppError> {
    let valid = dump.users.is_some() == dump.scopes.identities
        && dump.groups.is_some() == dump.scopes.identities
        && dump.group_members.is_some() == dump.scopes.identities
        && dump.machine_accounts.is_some() == dump.scopes.machine_accounts
        && dump.project_user_grants.is_some() == dump.scopes.access_policies
        && dump.project_group_grants.is_some() == dump.scopes.access_policies
        && dump.project_machine_grants.is_some() == dump.scopes.access_policies
        && dump.secret_user_grants.is_some() == dump.scopes.access_policies
        && dump.secret_group_grants.is_some() == dump.scopes.access_policies
        && dump.secret_machine_grants.is_some() == dump.scopes.access_policies
        && dump.machine_user_grants.is_some() == dump.scopes.access_policies
        && dump.machine_group_grants.is_some() == dump.scopes.access_policies
        && dump.audit_settings.is_some() == dump.scopes.audit
        && dump.audit_events.is_some() == dump.scopes.audit
        && dump.backup_targets.is_some() == dump.scopes.backup_targets;
    valid
        .then_some(())
        .ok_or_else(|| AppError::Validation("backup sections do not match declared scopes".into()))
}

async fn clear_full_instance(connection: &impl ConnectionTrait) -> Result<(), AppError> {
    connection
        .execute_unprepared(
            r#"
            UPDATE audit_settings SET cleanup_authorized = 1 WHERE id = 1;
            DELETE FROM audit_events;
            DELETE FROM project_user_grants;
            DELETE FROM project_group_grants;
            DELETE FROM project_machine_grants;
            DELETE FROM secret_user_grants;
            DELETE FROM secret_group_grants;
            DELETE FROM secret_machine_grants;
            DELETE FROM machine_user_grants;
            DELETE FROM machine_group_grants;
            DELETE FROM backup_jobs;
            DELETE FROM backup_targets;
            DELETE FROM machine_sessions;
            DELETE FROM machine_accounts;
            DELETE FROM group_members;
            DELETE FROM groups;
            DELETE FROM sessions;
            DELETE FROM secrets;
            DELETE FROM projects;
            DELETE FROM users;
            DELETE FROM audit_settings;
            "#,
        )
        .await?;
    Ok(())
}

async fn upsert_models<A, M>(
    connection: &impl ConnectionTrait,
    models: Vec<M>,
) -> Result<(), AppError>
where
    A: ActiveModelTrait + sea_orm::ActiveModelBehavior + Clone + Send,
    A::Entity: EntityTrait,
    <A::Entity as EntityTrait>::Model: IntoActiveModel<A>,
    M: IntoActiveModel<A>,
{
    for model in models {
        upsert_model(connection, model.into_active_model()).await?;
    }
    Ok(())
}

async fn upsert_model<A>(connection: &impl ConnectionTrait, active: A) -> Result<(), AppError>
where
    A: ActiveModelTrait + sea_orm::ActiveModelBehavior + Clone + Send,
    A::Entity: EntityTrait,
    <A::Entity as EntityTrait>::Model: IntoActiveModel<A>,
{
    match active.clone().reset_all().update(connection).await {
        Ok(_) => Ok(()),
        Err(DbErr::RecordNotUpdated | DbErr::RecordNotFound(_)) => {
            active.insert(connection).await?;
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

pub fn encrypt_portable(passphrase: &str, plaintext: &[u8]) -> Result<Vec<u8>, AppError> {
    validate_passphrase(passphrase)?;
    let mut salt = [0_u8; SALT_LEN];
    let mut nonce = [0_u8; NONCE_LEN];
    let mut rng = rand::rng();
    rng.try_fill_bytes(&mut salt).map_err(AppError::internal)?;
    rng.try_fill_bytes(&mut nonce).map_err(AppError::internal)?;
    let key = derive_key(passphrase, &salt)?;
    let ciphertext = Aes256Gcm::new_from_slice(&key)
        .expect("AES-256 key length")
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad: EXPORT_MAGIC,
            },
        )
        .map_err(|_| AppError::internal(anyhow::anyhow!("export encryption failed")))?;
    let mut output =
        Vec::with_capacity(EXPORT_MAGIC.len() + SALT_LEN + NONCE_LEN + ciphertext.len());
    output.extend_from_slice(EXPORT_MAGIC);
    output.extend_from_slice(&salt);
    output.extend_from_slice(&nonce);
    output.extend_from_slice(&ciphertext);
    Ok(output)
}

pub fn decrypt_portable(passphrase: &str, envelope: &[u8]) -> Result<Vec<u8>, AppError> {
    validate_passphrase(passphrase)?;
    let header_len = EXPORT_MAGIC.len() + SALT_LEN + NONCE_LEN;
    if envelope.len() <= header_len || &envelope[..EXPORT_MAGIC.len()] != EXPORT_MAGIC {
        return Err(AppError::Validation("invalid LightBWS export".into()));
    }
    let salt_start = EXPORT_MAGIC.len();
    let nonce_start = salt_start + SALT_LEN;
    let body_start = nonce_start + NONCE_LEN;
    let key = derive_key(passphrase, &envelope[salt_start..nonce_start])?;
    Aes256Gcm::new_from_slice(&key)
        .expect("AES-256 key length")
        .decrypt(
            Nonce::from_slice(&envelope[nonce_start..body_start]),
            Payload {
                msg: &envelope[body_start..],
                aad: EXPORT_MAGIC,
            },
        )
        .map_err(|_| AppError::Validation("export passphrase or file is invalid".into()))
}

pub fn encrypt_backup(master_key: &MasterKey, plaintext: &[u8]) -> Result<Vec<u8>, AppError> {
    let encrypted = master_key
        .encrypt(BACKUP_AAD, plaintext)
        .map_err(AppError::internal)?;
    Ok(format!("{}{encrypted}\n", String::from_utf8_lossy(AUTOMATIC_MAGIC)).into_bytes())
}

pub fn decrypt_backup(master_key: &MasterKey, envelope: &[u8]) -> Result<Vec<u8>, AppError> {
    if !envelope.starts_with(AUTOMATIC_MAGIC) {
        return Err(AppError::Validation("invalid automatic backup".into()));
    }
    let encoded = std::str::from_utf8(&envelope[AUTOMATIC_MAGIC.len()..])
        .map_err(|_| AppError::Validation("invalid automatic backup".into()))?
        .trim();
    master_key
        .decrypt(BACKUP_AAD, encoded)
        .map_err(|_| AppError::Validation("master key or backup file is invalid".into()))
}

pub fn encode_plain_backup(plaintext: &[u8]) -> Result<Vec<u8>, AppError> {
    if plaintext.len() > MAX_EXPORT_BYTES {
        return Err(AppError::Validation(
            "database export exceeds the 64 MiB safety limit".into(),
        ));
    }
    let mut output = Vec::with_capacity(PLAIN_MAGIC.len() + plaintext.len());
    output.extend_from_slice(PLAIN_MAGIC);
    output.extend_from_slice(plaintext);
    Ok(output)
}

pub fn decode_plain_backup(envelope: &[u8]) -> Result<Vec<u8>, AppError> {
    if !envelope.starts_with(PLAIN_MAGIC) {
        return Err(AppError::Validation("invalid plaintext backup".into()));
    }
    let plaintext = &envelope[PLAIN_MAGIC.len()..];
    if plaintext.is_empty() || plaintext.len() > MAX_EXPORT_BYTES {
        return Err(AppError::Validation("invalid plaintext backup".into()));
    }
    Ok(plaintext.to_vec())
}

fn validate_passphrase(value: &str) -> Result<(), AppError> {
    if !(12..=4096).contains(&value.chars().count()) || value.chars().any(char::is_control) {
        return Err(AppError::Validation(
            "export passphrase must contain 12-4096 non-control characters".into(),
        ));
    }
    Ok(())
}

fn derive_key(passphrase: &str, salt: &[u8]) -> Result<[u8; 32], AppError> {
    let mut key = [0_u8; 32];
    let params = Params::new(32 * 1024, 3, 1, Some(32)).map_err(AppError::internal)?;
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(AppError::internal)?;
    Ok(key)
}

fn validate_project_record(record: &ProjectRecord) -> Result<(), AppError> {
    uuid::Uuid::parse_str(&record.id)
        .map_err(|_| AppError::Validation("invalid project identifier".into()))?;
    if record
        .name_plain
        .as_deref()
        .is_some_and(|value| value.trim().is_empty() || value.chars().count() > 500)
        || record
            .name_cipher
            .as_deref()
            .is_some_and(|value| value.len() > 16_384)
        || (record.name_plain.is_some() == record.name_cipher.is_some())
    {
        return Err(AppError::Validation("invalid project record".into()));
    }
    Ok(())
}

fn validate_secret_record(record: &SecretRecord) -> Result<(), AppError> {
    uuid::Uuid::parse_str(&record.id)
        .map_err(|_| AppError::Validation("invalid secret identifier".into()))?;
    if uuid::Uuid::parse_str(&record.project_id).is_err()
        || record
            .key_plain
            .as_deref()
            .is_some_and(|value| value.trim().is_empty() || value.chars().count() > 500)
        || record
            .value_plain
            .as_deref()
            .is_some_and(|value| value.len() > 1024 * 1024)
        || record
            .note_plain
            .as_deref()
            .is_some_and(|value| value.len() > 64 * 1024)
        || record
            .key_cipher
            .as_deref()
            .is_some_and(|value| value.is_empty() || value.len() > 32 * 1024)
        || record
            .value_cipher
            .as_deref()
            .is_some_and(|value| value.is_empty() || value.len() > 2 * 1024 * 1024)
        || record
            .note_cipher
            .as_deref()
            .is_some_and(|value| value.len() > 128 * 1024)
        || !valid_secret_mode(record)
    {
        return Err(AppError::Validation("invalid secret record".into()));
    }
    Ok(())
}

fn valid_secret_mode(record: &SecretRecord) -> bool {
    let cipher = record.key_cipher.is_some()
        && record.value_cipher.is_some()
        && record.note_cipher.is_some()
        && record.key_plain.is_none()
        && record.value_plain.is_none()
        && record.note_plain.is_none();
    let plain = record.key_cipher.is_none()
        && record.value_cipher.is_none()
        && record.note_cipher.is_none()
        && record.key_plain.is_some()
        && record.value_plain.is_some()
        && record.note_plain.is_some();
    cipher || plain
}

fn apply_secret(
    active: &mut secret::ActiveModel,
    record: SecretRecord,
    import_revision: Option<i64>,
) {
    active.project_id = Set(record.project_id);
    active.key_cipher = Set(record.key_cipher);
    active.value_cipher = Set(record.value_cipher);
    active.note_cipher = Set(record.note_cipher);
    active.key_plain = Set(record.key_plain);
    active.value_plain = Set(record.value_plain);
    active.note_plain = Set(record.note_plain);
    active.deleted_at = Set(record.deleted_at);
    active.created_at = Set(record.created_at);
    active.updated_at = Set(record.updated_at);
    active.revision_nanos = Set(import_revision
        .map(|revision| revision.max(record.revision_nanos))
        .unwrap_or(record.revision_nanos));
}

impl From<project::Model> for ProjectRecord {
    fn from(value: project::Model) -> Self {
        Self {
            id: value.id,
            organization_id: value.organization_id,
            name_cipher: value.name_cipher,
            name_plain: value.name_plain,
            deleted_at: value.deleted_at,
            created_at: value.created_at,
            updated_at: value.updated_at,
            revision_nanos: value.revision_nanos,
        }
    }
}

impl From<secret::Model> for SecretRecord {
    fn from(value: secret::Model) -> Self {
        Self {
            id: value.id,
            organization_id: ORGANIZATION_ID.into(),
            project_id: value.project_id,
            key_cipher: value.key_cipher,
            value_cipher: value.value_cipher,
            note_cipher: value.note_cipher,
            key_plain: value.key_plain,
            value_plain: value.value_plain,
            note_plain: value.note_plain,
            deleted_at: value.deleted_at,
            created_at: value.created_at,
            updated_at: value.updated_at,
            revision_nanos: value.revision_nanos,
        }
    }
}
