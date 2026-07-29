use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    time::Duration,
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use hmac::{Hmac, Mac};
use reqwest::{Client, Method, StatusCode, Url, header};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter,
    QueryOrder, QuerySelect, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    AppState,
    crypto::MasterKey,
    db::{
        Database,
        entities::{backup_job, backup_target},
    },
    domain::{
        now,
        transfer::{BackupScopes, dump_database_scoped, encode_plain_backup, encrypt_backup},
    },
    error::AppError,
};

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "settings",
    rename_all = "SCREAMING_SNAKE_CASE",
    deny_unknown_fields
)]
pub enum BackupPublicConfig {
    S3(S3PublicConfig),
    Webdav(WebDavPublicConfig),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct S3PublicConfig {
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    #[serde(default)]
    pub prefix: String,
    #[serde(default)]
    pub path_style: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebDavPublicConfig {
    pub endpoint: String,
    #[serde(default)]
    pub prefix: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(
    tag = "kind",
    content = "values",
    rename_all = "SCREAMING_SNAKE_CASE",
    deny_unknown_fields
)]
pub enum BackupCredentials {
    S3(S3Credentials),
    Webdav(WebDavCredentials),
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct S3Credentials {
    access_key_id: String,
    secret_access_key: String,
    session_token: Option<String>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WebDavCredentials {
    username: String,
    password: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupTarget {
    pub id: Uuid,
    pub display_name: String,
    pub config: BackupPublicConfig,
    pub enabled: bool,
    pub schedule_enabled: bool,
    pub interval_hours: u16,
    pub next_run_at: Option<i64>,
    pub last_run_at: Option<i64>,
    pub last_status: Option<String>,
    pub last_error: Option<String>,
    pub has_credentials: bool,
    pub scopes: BackupScopes,
    pub encryption: BackupEncryption,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BackupEncryption {
    #[default]
    MasterKey,
    Plaintext,
}

impl BackupEncryption {
    fn as_str(self) -> &'static str {
        match self {
            Self::MasterKey => "master_key",
            Self::Plaintext => "plaintext",
        }
    }

    fn parse(value: &str) -> Result<Self, AppError> {
        match value {
            "master_key" => Ok(Self::MasterKey),
            "plaintext" => Ok(Self::Plaintext),
            _ => Err(AppError::internal(anyhow::anyhow!(
                "invalid stored backup encryption mode"
            ))),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupJob {
    pub id: Uuid,
    pub target_id: Uuid,
    pub trigger_kind: String,
    pub status: String,
    pub object_key: String,
    pub byte_size: Option<i64>,
    pub error_code: Option<String>,
    pub created_at: i64,
    pub completed_at: Option<i64>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateBackupTarget {
    pub display_name: String,
    pub config: BackupPublicConfig,
    pub credentials: BackupCredentials,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub schedule_enabled: bool,
    #[serde(default = "default_interval")]
    pub interval_hours: u16,
    #[serde(default)]
    pub scopes: BackupScopes,
    #[serde(default)]
    pub encryption: BackupEncryption,
    #[serde(default)]
    pub confirm_plaintext: bool,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateBackupTarget {
    pub display_name: String,
    pub config: BackupPublicConfig,
    pub credentials: Option<BackupCredentials>,
    pub enabled: bool,
    pub schedule_enabled: bool,
    pub interval_hours: u16,
    #[serde(default)]
    pub scopes: BackupScopes,
    #[serde(default)]
    pub encryption: BackupEncryption,
    #[serde(default)]
    pub confirm_plaintext: bool,
}

#[derive(Clone)]
pub struct BackupRepository {
    db: Database,
    master_key: MasterKey,
    allow_plaintext: bool,
}

impl BackupRepository {
    pub fn new(db: Database, master_key: MasterKey) -> Self {
        Self {
            db,
            master_key,
            allow_plaintext: false,
        }
    }

    pub fn with_plaintext_allowed(mut self, allowed: bool) -> Self {
        self.allow_plaintext = allowed;
        self
    }

    pub fn for_state(state: &AppState) -> Self {
        Self::new(state.db.clone(), state.master_key.clone())
            .with_plaintext_allowed(state.allow_plaintext_backups)
    }

    pub async fn list_targets(&self) -> Result<Vec<BackupTarget>, AppError> {
        backup_target::Entity::find()
            .order_by_asc(backup_target::Column::DisplayName)
            .all(self.db.connection())
            .await?
            .into_iter()
            .map(BackupTarget::try_from)
            .collect()
    }

    pub async fn get_target(&self, id: Uuid) -> Result<BackupTarget, AppError> {
        BackupTarget::try_from(self.get_model(id).await?)
    }

    pub async fn create_target(&self, input: CreateBackupTarget) -> Result<BackupTarget, AppError> {
        let display_name = normalize_display_name(&input.display_name)?;
        let config = normalize_public_config(input.config)?;
        validate_credentials(&config, &input.credentials)?;
        validate_interval(input.interval_hours)?;
        validate_backup_options(input.scopes, input.encryption, self.allow_plaintext)?;
        validate_plaintext_confirmation(input.encryption, input.confirm_plaintext)?;
        let id = Uuid::new_v4();
        let timestamp = now();
        let credentials = encode_credentials(&input.credentials)?;
        let model = backup_target::ActiveModel {
            id: Set(id.to_string()),
            display_name: Set(display_name),
            kind: Set(config.kind().into()),
            public_config_json: Set(serde_json::to_string(&config).map_err(AppError::internal)?),
            credentials_cipher: Set(self
                .master_key
                .encrypt(id.as_bytes(), &credentials)
                .map_err(AppError::internal)?),
            scopes_json: Set(serde_json::to_string(&input.scopes).map_err(AppError::internal)?),
            encryption_mode: Set(input.encryption.as_str().into()),
            enabled: Set(input.enabled),
            schedule_enabled: Set(input.schedule_enabled),
            interval_hours: Set(i32::from(input.interval_hours)),
            next_run_at: Set((input.enabled && input.schedule_enabled)
                .then_some(timestamp + i64::from(input.interval_hours) * 3600)),
            last_run_at: Set(None),
            last_status: Set(None),
            last_error: Set(None),
            created_at: Set(timestamp),
            updated_at: Set(timestamp),
        }
        .insert(self.db.connection())
        .await
        .map_err(conflict_or_internal)?;
        BackupTarget::try_from(model)
    }

    pub async fn update_target(
        &self,
        id: Uuid,
        input: UpdateBackupTarget,
    ) -> Result<BackupTarget, AppError> {
        let display_name = normalize_display_name(&input.display_name)?;
        let config = normalize_public_config(input.config)?;
        validate_interval(input.interval_hours)?;
        validate_backup_options(input.scopes, input.encryption, self.allow_plaintext)?;
        validate_plaintext_confirmation(input.encryption, input.confirm_plaintext)?;
        let model = self.get_model(id).await?;
        let mut active = model.into_active_model();
        active.display_name = Set(display_name);
        active.kind = Set(config.kind().into());
        active.public_config_json =
            Set(serde_json::to_string(&config).map_err(AppError::internal)?);
        if let Some(credentials) = input.credentials {
            validate_credentials(&config, &credentials)?;
            active.credentials_cipher = Set(self
                .master_key
                .encrypt(id.as_bytes(), &encode_credentials(&credentials)?)
                .map_err(AppError::internal)?);
        }
        active.scopes_json = Set(serde_json::to_string(&input.scopes).map_err(AppError::internal)?);
        active.encryption_mode = Set(input.encryption.as_str().into());
        active.enabled = Set(input.enabled);
        active.schedule_enabled = Set(input.schedule_enabled);
        active.interval_hours = Set(i32::from(input.interval_hours));
        active.next_run_at = Set((input.enabled && input.schedule_enabled)
            .then_some(now() + i64::from(input.interval_hours) * 3600));
        active.updated_at = Set(now());
        BackupTarget::try_from(
            active
                .update(self.db.connection())
                .await
                .map_err(conflict_or_internal)?,
        )
    }

    pub async fn delete_target(&self, id: Uuid) -> Result<(), AppError> {
        let result = backup_target::Entity::delete_by_id(id.to_string())
            .exec(self.db.connection())
            .await?;
        if result.rows_affected == 0 {
            return Err(AppError::NotFound);
        }
        Ok(())
    }

    pub async fn list_jobs(&self) -> Result<Vec<BackupJob>, AppError> {
        backup_job::Entity::find()
            .order_by_desc(backup_job::Column::CreatedAt)
            .limit(100)
            .all(self.db.connection())
            .await?
            .into_iter()
            .map(BackupJob::try_from)
            .collect()
    }

    pub async fn due_target_ids(&self) -> Result<Vec<Uuid>, AppError> {
        backup_target::Entity::find()
            .filter(backup_target::Column::Enabled.eq(true))
            .filter(backup_target::Column::ScheduleEnabled.eq(true))
            .filter(backup_target::Column::NextRunAt.lte(now()))
            .all(self.db.connection())
            .await?
            .into_iter()
            .map(|model| Uuid::parse_str(&model.id).map_err(AppError::internal))
            .collect()
    }

    async fn get_model(&self, id: Uuid) -> Result<backup_target::Model, AppError> {
        backup_target::Entity::find_by_id(id.to_string())
            .one(self.db.connection())
            .await?
            .ok_or(AppError::NotFound)
    }

    async fn execution_target(&self, id: Uuid) -> Result<ExecutionTarget, AppError> {
        let model = self.get_model(id).await?;
        if !model.enabled {
            return Err(AppError::Conflict);
        }
        let config: BackupPublicConfig =
            serde_json::from_str(&model.public_config_json).map_err(AppError::internal)?;
        let plaintext = self
            .master_key
            .decrypt(id.as_bytes(), &model.credentials_cipher)
            .map_err(AppError::internal)?;
        let credentials = decode_credentials(&model.kind, &plaintext)?;
        let scopes: BackupScopes =
            serde_json::from_str(&model.scopes_json).map_err(AppError::internal)?;
        scopes.validate()?;
        let encryption = BackupEncryption::parse(&model.encryption_mode)?;
        validate_backup_options(scopes, encryption, self.allow_plaintext)?;
        Ok(ExecutionTarget {
            model,
            config,
            credentials,
            scopes,
            encryption,
        })
    }
}

struct ExecutionTarget {
    model: backup_target::Model,
    config: BackupPublicConfig,
    credentials: BackupCredentials,
    scopes: BackupScopes,
    encryption: BackupEncryption,
}

pub async fn run_backup(
    state: &AppState,
    target_id: Uuid,
    trigger: &str,
) -> Result<BackupJob, AppError> {
    let _permit = state
        .backup_permits
        .acquire()
        .await
        .map_err(AppError::internal)?;
    let repository = BackupRepository::for_state(state);
    let target = repository.execution_target(target_id).await?;
    let job_id = Uuid::new_v4();
    let timestamp = now();
    let suffix = if target.encryption == BackupEncryption::Plaintext {
        ".plain.lightbws"
    } else {
        ".lightbws"
    };
    let object_key = format!(
        "lightbws/{:04}/{:02}/{}-{}{}",
        time::OffsetDateTime::now_utc().year(),
        u8::from(time::OffsetDateTime::now_utc().month()),
        timestamp,
        job_id,
        suffix
    );
    let job = backup_job::ActiveModel {
        id: Set(job_id.to_string()),
        target_id: Set(target_id.to_string()),
        trigger_kind: Set(trigger.into()),
        status: Set("running".into()),
        object_key: Set(object_key.clone()),
        byte_size: Set(None),
        error_code: Set(None),
        created_at: Set(timestamp),
        completed_at: Set(None),
    }
    .insert(state.db.connection())
    .await
    .map_err(conflict_or_internal)?;
    let result = async {
        let dump = dump_database_scoped(&state.db, &state.master_key, target.scopes).await?;
        let payload = match target.encryption {
            BackupEncryption::MasterKey => encrypt_backup(&state.master_key, &dump)?,
            BackupEncryption::Plaintext => encode_plain_backup(&dump)?,
        };
        upload(&target.config, &target.credentials, &object_key, &payload).await?;
        Ok::<usize, AppError>(payload.len())
    }
    .await;
    let completed_at = now();
    let mut active_job = job.into_active_model();
    let mut active_target = target.model.into_active_model();
    active_target.last_run_at = Set(Some(completed_at));
    active_target.next_run_at = Set((*active_target.enabled.as_ref()
        && *active_target.schedule_enabled.as_ref())
    .then_some(completed_at + i64::from(*active_target.interval_hours.as_ref()) * 3600));
    active_target.updated_at = Set(completed_at);
    match result {
        Ok(byte_size) => {
            active_job.status = Set("succeeded".into());
            active_job.byte_size = Set(Some(i64::try_from(byte_size).map_err(AppError::internal)?));
            active_target.last_status = Set(Some("succeeded".into()));
            active_target.last_error = Set(None);
        }
        Err(error) => {
            let code = backup_error_code(&error).to_owned();
            tracing::warn!(target_id = %target_id, code, "backup failed");
            active_job.status = Set("failed".into());
            active_job.error_code = Set(Some(code.clone()));
            active_target.last_status = Set(Some("failed".into()));
            active_target.last_error = Set(Some(code));
        }
    }
    active_job.completed_at = Set(Some(completed_at));
    let transaction = state.db.connection().begin().await?;
    let stored = active_job.update(&transaction).await?;
    active_target.update(&transaction).await?;
    transaction.commit().await?;
    BackupJob::try_from(stored)
}

pub async fn recover_interrupted_jobs(db: &Database) -> Result<(), AppError> {
    use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

    let timestamp = now();
    let transaction = db.connection().begin().await?;
    transaction
        .execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"
            UPDATE backup_targets
            SET last_run_at = ?, last_status = 'failed', last_error = 'interrupted', updated_at = ?
            WHERE id IN (SELECT target_id FROM backup_jobs WHERE status = 'running')
            "#,
            [timestamp.into(), timestamp.into()],
        ))
        .await?;
    transaction
        .execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"
            UPDATE backup_jobs
            SET status = 'failed', error_code = 'interrupted', completed_at = ?
            WHERE status = 'running'
            "#,
            [timestamp.into()],
        ))
        .await?;
    transaction.commit().await?;
    Ok(())
}

pub async fn test_target(state: &AppState, target_id: Uuid) -> Result<(), AppError> {
    let target = BackupRepository::for_state(state)
        .execution_target(target_id)
        .await?;
    let endpoint = Url::parse(target.config.endpoint()).map_err(AppError::internal)?;
    let _ = secure_client(&endpoint).await?;
    Ok(())
}

pub async fn scheduler(state: AppState) {
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        let repository = BackupRepository::for_state(&state);
        match repository.due_target_ids().await {
            Ok(ids) => {
                for id in ids {
                    if let Err(error) = run_backup(&state, id, "scheduled").await {
                        tracing::warn!(target_id = %id, code = backup_error_code(&error), "scheduled backup failed");
                    }
                }
            }
            Err(error) => tracing::warn!(
                code = backup_error_code(&error),
                "backup scheduler query failed"
            ),
        }
    }
}

async fn upload(
    config: &BackupPublicConfig,
    credentials: &BackupCredentials,
    object_key: &str,
    body: &[u8],
) -> Result<(), AppError> {
    match (config, credentials) {
        (BackupPublicConfig::S3(config), BackupCredentials::S3(credentials)) => {
            upload_s3(config, credentials, object_key, body).await
        }
        (BackupPublicConfig::Webdav(config), BackupCredentials::Webdav(credentials)) => {
            upload_webdav(config, credentials, object_key, body).await
        }
        _ => Err(AppError::Validation(
            "backup credentials do not match target kind".into(),
        )),
    }
}

async fn upload_s3(
    config: &S3PublicConfig,
    credentials: &S3Credentials,
    object_key: &str,
    body: &[u8],
) -> Result<(), AppError> {
    let mut url = Url::parse(&config.endpoint).map_err(AppError::internal)?;
    let endpoint_host = url
        .host_str()
        .ok_or_else(|| AppError::Validation("backup endpoint has no host".into()))?
        .to_owned();
    if config.path_style {
        url.set_path(&format!(
            "/{}/{}",
            config.bucket,
            join_prefix(&config.prefix, object_key)
        ));
    } else {
        url.set_host(Some(&format!("{}.{}", config.bucket, endpoint_host)))
            .map_err(|_| AppError::Validation("invalid S3 bucket host".into()))?;
        url.set_path(&format!("/{}", join_prefix(&config.prefix, object_key)));
    }
    let client = secure_client(&url).await?;
    let timestamp = time::OffsetDateTime::now_utc();
    let date = format!(
        "{:04}{:02}{:02}",
        timestamp.year(),
        u8::from(timestamp.month()),
        timestamp.day()
    );
    let amz_date = format!(
        "{date}T{:02}{:02}{:02}Z",
        timestamp.hour(),
        timestamp.minute(),
        timestamp.second()
    );
    let payload_hash = hex::encode(Sha256::digest(body));
    let host = authority(&url)?;
    let mut canonical_headers = format!(
        "content-type:application/vnd.lightbws.backup\nhost:{host}\nx-amz-content-sha256:{payload_hash}\nx-amz-date:{amz_date}\n"
    );
    let mut signed_headers = "content-type;host;x-amz-content-sha256;x-amz-date".to_owned();
    if let Some(token) = credentials.session_token.as_deref() {
        canonical_headers.push_str(&format!("x-amz-security-token:{}\n", token.trim()));
        signed_headers.push_str(";x-amz-security-token");
    }
    let canonical_request = format!(
        "PUT\n{}\n\n{}\n{}\n{}",
        url.path(),
        canonical_headers,
        signed_headers,
        payload_hash
    );
    let scope = format!("{date}/{}/s3/aws4_request", config.region);
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
        hex::encode(Sha256::digest(canonical_request.as_bytes()))
    );
    let signing_key = aws_signing_key(&credentials.secret_access_key, &date, &config.region)?;
    let signature = hex::encode(hmac(&signing_key, string_to_sign.as_bytes())?);
    let authorization = format!(
        "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed_headers}, Signature={signature}",
        credentials.access_key_id
    );
    let mut request = client
        .put(url)
        .header(header::CONTENT_TYPE, "application/vnd.lightbws.backup")
        .header("x-amz-content-sha256", payload_hash)
        .header("x-amz-date", amz_date)
        .header(header::AUTHORIZATION, authorization)
        .body(body.to_vec());
    if let Some(token) = &credentials.session_token {
        request = request.header("x-amz-security-token", token);
    }
    let response = request
        .send()
        .await
        .map_err(|_| AppError::internal(anyhow::anyhow!("S3 transport failed")))?;
    require_remote_success(response.status())
}

async fn upload_webdav(
    config: &WebDavPublicConfig,
    credentials: &WebDavCredentials,
    object_key: &str,
    body: &[u8],
) -> Result<(), AppError> {
    let base = Url::parse(&config.endpoint).map_err(AppError::internal)?;
    let client = secure_client(&base).await?;
    let authorization = format!(
        "Basic {}",
        STANDARD.encode(format!("{}:{}", credentials.username, credentials.password))
    );
    let full_key = join_prefix(&config.prefix, object_key);
    let mut parts = Vec::new();
    for segment in full_key
        .split('/')
        .filter(|segment| !segment.is_empty())
        .take(
            full_key
                .split('/')
                .filter(|segment| !segment.is_empty())
                .count()
                .saturating_sub(1),
        )
    {
        parts.push(segment);
        let url = append_url_path(&base, &parts)?;
        let response = client
            .request(
                Method::from_bytes(b"MKCOL").map_err(AppError::internal)?,
                url,
            )
            .header(header::AUTHORIZATION, &authorization)
            .send()
            .await
            .map_err(|_| AppError::internal(anyhow::anyhow!("WebDAV transport failed")))?;
        if !matches!(response.status().as_u16(), 200..=299 | 405) {
            return Err(AppError::internal(anyhow::anyhow!(
                "WebDAV collection creation failed"
            )));
        }
    }
    let url = append_url_path(
        &base,
        &full_key
            .split('/')
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>(),
    )?;
    let response = client
        .put(url)
        .header(header::AUTHORIZATION, authorization)
        .header(header::CONTENT_TYPE, "application/vnd.lightbws.backup")
        .body(body.to_vec())
        .send()
        .await
        .map_err(|_| AppError::internal(anyhow::anyhow!("WebDAV transport failed")))?;
    require_remote_success(response.status())
}

async fn secure_client(url: &Url) -> Result<Client, AppError> {
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(AppError::Validation(
            "backup endpoints must be credential-free HTTPS URLs".into(),
        ));
    }
    let host = url
        .host_str()
        .ok_or_else(|| AppError::Validation("backup endpoint has no host".into()))?;
    let port = url
        .port_or_known_default()
        .ok_or_else(|| AppError::Validation("backup endpoint has no port".into()))?;
    let addresses = tokio::net::lookup_host((host, port))
        .await
        .map_err(|_| AppError::Validation("backup endpoint cannot be resolved".into()))?
        .collect::<Vec<_>>();
    if addresses.is_empty() || addresses.iter().any(|address| !is_public_ip(address.ip())) {
        return Err(AppError::Validation(
            "backup endpoint must resolve only to public addresses".into(),
        ));
    }
    let pinned = SocketAddr::new(addresses[0].ip(), port);
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .resolve(host, pinned)
        .build()
        .map_err(AppError::internal)
}

fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_ipv4(ip),
        IpAddr::V6(ip) => is_public_ipv6(ip),
    }
}

fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_multicast()
        || ip.is_broadcast()
        || ip.is_unspecified()
        || a == 0
        || (a == 100 && (64..=127).contains(&b))
        || (a == 192 && b == 0 && c == 0)
        || (a == 192 && b == 0 && c == 2)
        || (a == 198 && matches!(b, 18 | 19))
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113)
        || a >= 240)
}

fn is_public_ipv6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    !(ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || (segments[0] & 0xfe00) == 0xfc00
        || (segments[0] & 0xffc0) == 0xfe80
        || (segments[0] == 0x2001 && segments[1] == 0x0db8))
}

fn normalize_public_config(config: BackupPublicConfig) -> Result<BackupPublicConfig, AppError> {
    match config {
        BackupPublicConfig::S3(mut value) => {
            value.endpoint = normalize_endpoint(&value.endpoint, true)?;
            value.region = normalize_token(&value.region, 1, 64, "S3 region")?;
            value.bucket = normalize_bucket(&value.bucket)?;
            value.prefix = normalize_prefix(&value.prefix)?;
            Ok(BackupPublicConfig::S3(value))
        }
        BackupPublicConfig::Webdav(mut value) => {
            value.endpoint = normalize_endpoint(&value.endpoint, false)?;
            value.prefix = normalize_prefix(&value.prefix)?;
            Ok(BackupPublicConfig::Webdav(value))
        }
    }
}

fn normalize_endpoint(value: &str, origin_only: bool) -> Result<String, AppError> {
    let mut url = Url::parse(value.trim())
        .map_err(|_| AppError::Validation("invalid backup endpoint".into()))?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(AppError::Validation(
            "backup endpoint must be a credential-free HTTPS URL".into(),
        ));
    }
    if origin_only && !matches!(url.path(), "" | "/") {
        return Err(AppError::Validation(
            "S3 endpoint must not contain a path".into(),
        ));
    }
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path());
        url.set_path(&path);
    }
    Ok(url.to_string().trim_end_matches('/').to_owned())
}

fn normalize_display_name(value: &str) -> Result<String, AppError> {
    let value = value.trim();
    if !(1..=128).contains(&value.chars().count()) || value.chars().any(char::is_control) {
        return Err(AppError::Validation("invalid backup target name".into()));
    }
    Ok(value.into())
}

fn normalize_token(value: &str, min: usize, max: usize, field: &str) -> Result<String, AppError> {
    let value = value.trim();
    if !(min..=max).contains(&value.len())
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(AppError::Validation(format!("invalid {field}")));
    }
    Ok(value.into())
}

fn normalize_bucket(value: &str) -> Result<String, AppError> {
    let bucket = normalize_token(value, 3, 63, "S3 bucket")?;
    if bucket.starts_with('-') || bucket.ends_with('-') || bucket.contains("..") {
        return Err(AppError::Validation("invalid S3 bucket".into()));
    }
    Ok(bucket)
}

fn normalize_prefix(value: &str) -> Result<String, AppError> {
    let value = value.trim().trim_matches('/');
    if value.len() > 512
        || value.split('/').any(|segment| {
            segment.is_empty()
                || segment == "."
                || segment == ".."
                || segment.contains('\\')
                || segment.chars().any(char::is_control)
        })
    {
        return Err(AppError::Validation("invalid backup prefix".into()));
    }
    Ok(value.into())
}

fn validate_credentials(
    config: &BackupPublicConfig,
    credentials: &BackupCredentials,
) -> Result<(), AppError> {
    match (config, credentials) {
        (BackupPublicConfig::S3(_), BackupCredentials::S3(value)) => {
            validate_secret(&value.access_key_id, 1, 256)?;
            validate_secret(&value.secret_access_key, 1, 512)?;
            if let Some(token) = &value.session_token {
                validate_secret(token, 1, 4096)?;
            }
        }
        (BackupPublicConfig::Webdav(_), BackupCredentials::Webdav(value)) => {
            validate_secret(&value.username, 1, 512)?;
            validate_secret(&value.password, 1, 4096)?;
        }
        _ => {
            return Err(AppError::Validation(
                "backup credentials do not match target kind".into(),
            ));
        }
    }
    Ok(())
}

fn validate_secret(value: &str, min: usize, max: usize) -> Result<(), AppError> {
    if !(min..=max).contains(&value.len()) || value.chars().any(char::is_control) {
        return Err(AppError::Validation("invalid backup credential".into()));
    }
    Ok(())
}

fn validate_interval(value: u16) -> Result<(), AppError> {
    if !(1..=720).contains(&value) {
        return Err(AppError::Validation(
            "backup interval must be 1-720 hours".into(),
        ));
    }
    Ok(())
}

pub(crate) fn encode_credentials(value: &BackupCredentials) -> Result<Vec<u8>, AppError> {
    let json = match value {
        BackupCredentials::S3(value) => serde_json::json!({ "kind": "s3", "values": value }),
        BackupCredentials::Webdav(value) => {
            serde_json::json!({ "kind": "webdav", "values": value })
        }
    };
    serde_json::to_vec(&json).map_err(AppError::internal)
}

pub(crate) fn decode_credentials(kind: &str, value: &[u8]) -> Result<BackupCredentials, AppError> {
    #[derive(Deserialize)]
    struct Stored<T> {
        values: T,
    }
    match kind {
        "s3" => serde_json::from_slice::<Stored<S3Credentials>>(value)
            .map(|value| BackupCredentials::S3(value.values))
            .map_err(AppError::internal),
        "webdav" => serde_json::from_slice::<Stored<WebDavCredentials>>(value)
            .map(|value| BackupCredentials::Webdav(value.values))
            .map_err(AppError::internal),
        _ => Err(AppError::internal(anyhow::anyhow!(
            "invalid stored backup target kind"
        ))),
    }
}

impl BackupPublicConfig {
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::S3(_) => "s3",
            Self::Webdav(_) => "webdav",
        }
    }
    fn endpoint(&self) -> &str {
        match self {
            Self::S3(value) => &value.endpoint,
            Self::Webdav(value) => &value.endpoint,
        }
    }
}

impl TryFrom<backup_target::Model> for BackupTarget {
    type Error = AppError;
    fn try_from(value: backup_target::Model) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Uuid::parse_str(&value.id).map_err(AppError::internal)?,
            display_name: value.display_name,
            config: serde_json::from_str(&value.public_config_json).map_err(AppError::internal)?,
            enabled: value.enabled,
            schedule_enabled: value.schedule_enabled,
            interval_hours: u16::try_from(value.interval_hours).map_err(AppError::internal)?,
            next_run_at: value.next_run_at,
            last_run_at: value.last_run_at,
            last_status: value.last_status,
            last_error: value.last_error,
            has_credentials: !value.credentials_cipher.is_empty(),
            scopes: serde_json::from_str(&value.scopes_json).map_err(AppError::internal)?,
            encryption: BackupEncryption::parse(&value.encryption_mode)?,
            created_at: value.created_at,
            updated_at: value.updated_at,
        })
    }
}

impl TryFrom<backup_job::Model> for BackupJob {
    type Error = AppError;
    fn try_from(value: backup_job::Model) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Uuid::parse_str(&value.id).map_err(AppError::internal)?,
            target_id: Uuid::parse_str(&value.target_id).map_err(AppError::internal)?,
            trigger_kind: value.trigger_kind,
            status: value.status,
            object_key: value.object_key,
            byte_size: value.byte_size,
            error_code: value.error_code,
            created_at: value.created_at,
            completed_at: value.completed_at,
        })
    }
}

fn join_prefix(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.into()
    } else {
        format!("{prefix}/{key}")
    }
}

fn append_url_path(base: &Url, segments: &[&str]) -> Result<Url, AppError> {
    let mut url = base.clone();
    let mut path = url.path().trim_end_matches('/').to_owned();
    for segment in segments {
        path.push('/');
        path.push_str(segment);
    }
    url.set_path(&path);
    Ok(url)
}

fn authority(url: &Url) -> Result<String, AppError> {
    let host = url
        .host_str()
        .ok_or_else(|| AppError::Validation("backup endpoint has no host".into()))?;
    Ok(match url.port() {
        Some(port) if port != 443 => format!("{host}:{port}"),
        _ => host.into(),
    })
}

fn aws_signing_key(secret: &str, date: &str, region: &str) -> Result<Vec<u8>, AppError> {
    let date_key = hmac(format!("AWS4{secret}").as_bytes(), date.as_bytes())?;
    let region_key = hmac(&date_key, region.as_bytes())?;
    let service_key = hmac(&region_key, b"s3")?;
    hmac(&service_key, b"aws4_request")
}

fn hmac(key: &[u8], value: &[u8]) -> Result<Vec<u8>, AppError> {
    let mut mac = HmacSha256::new_from_slice(key).map_err(AppError::internal)?;
    mac.update(value);
    Ok(mac.finalize().into_bytes().to_vec())
}

fn require_remote_success(status: StatusCode) -> Result<(), AppError> {
    if status.is_success() {
        Ok(())
    } else {
        Err(AppError::internal(anyhow::anyhow!(
            "remote backup target rejected the request"
        )))
    }
}

fn conflict_or_internal(error: sea_orm::DbErr) -> AppError {
    if error.to_string().contains("UNIQUE") {
        AppError::Conflict
    } else {
        error.into()
    }
}

fn backup_error_code(error: &AppError) -> &'static str {
    match error {
        AppError::Validation(_) => "TARGET_CONFIG",
        AppError::NotFound => "TARGET_NOT_FOUND",
        AppError::Conflict => "TARGET_DISABLED",
        _ => "TRANSPORT_FAILED",
    }
}

const fn default_true() -> bool {
    true
}
const fn default_interval() -> u16 {
    24
}

fn validate_backup_options(
    scopes: BackupScopes,
    encryption: BackupEncryption,
    allow_plaintext: bool,
) -> Result<(), AppError> {
    scopes.validate()?;
    if encryption == BackupEncryption::Plaintext && !allow_plaintext {
        return Err(AppError::Validation(
            "plaintext backups are disabled by server configuration".into(),
        ));
    }
    Ok(())
}

fn validate_plaintext_confirmation(
    encryption: BackupEncryption,
    confirmed: bool,
) -> Result<(), AppError> {
    if encryption == BackupEncryption::Plaintext && !confirmed {
        return Err(AppError::Validation(
            "plaintext backups require explicit confirmation".into(),
        ));
    }
    Ok(())
}

pub(crate) fn validate_restored_target(
    display_name: &str,
    config: BackupPublicConfig,
    credentials: &BackupCredentials,
    interval_hours: i32,
    scopes: BackupScopes,
    encryption: BackupEncryption,
    allow_plaintext: bool,
) -> Result<(String, BackupPublicConfig, u16), AppError> {
    let display_name = normalize_display_name(display_name)?;
    let config = normalize_public_config(config)?;
    validate_credentials(&config, credentials)?;
    let interval_hours = u16::try_from(interval_hours)
        .map_err(|_| AppError::Validation("backup interval is invalid".into()))?;
    validate_interval(interval_hours)?;
    validate_backup_options(scopes, encryption, allow_plaintext)?;
    Ok((display_name, config, interval_hours))
}

#[cfg(test)]
mod tests {
    use super::{is_public_ip, normalize_prefix};
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn rejects_internal_backup_addresses_and_unsafe_prefixes() {
        assert!(!is_public_ip(IpAddr::V4(Ipv4Addr::LOCALHOST)));
        assert!(!is_public_ip(IpAddr::V4(Ipv4Addr::new(10, 1, 2, 3))));
        assert!(is_public_ip(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))));
        assert!(normalize_prefix("safe/daily").is_ok());
        assert!(normalize_prefix("safe/../escape").is_err());
    }
}
