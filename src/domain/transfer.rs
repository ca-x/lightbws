use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, Payload},
};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::TryRng;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ConnectionTrait, DatabaseBackend, EntityTrait,
    IntoActiveModel, Statement, TransactionTrait,
};
use serde::{Deserialize, Serialize};

use crate::{
    crypto::MasterKey,
    db::{
        Database,
        entities::{project, secret},
    },
    domain::{ORGANIZATION_ID, next_sdk_revision},
    error::AppError,
};

const EXPORT_MAGIC: &[u8; 8] = b"LBWSX01\0";
const BACKUP_AAD: &[u8] = b"lightbws-backup-v1";
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const MAX_EXPORT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DatabaseDump {
    version: u8,
    exported_at: i64,
    projects: Vec<ProjectRecord>,
    secrets: Vec<SecretRecord>,
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

pub async fn import_database(db: &Database, bytes: &[u8]) -> Result<ImportSummary, AppError> {
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
    })
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
    Ok(format!("LIGHTBWS-BACKUP-V1\n{encrypted}\n").into_bytes())
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
