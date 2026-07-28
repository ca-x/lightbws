use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    db::entities::{project, secret},
    error::AppError,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SdkProject {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub creation_date: String,
    pub revision_date: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SdkSecret {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub project_id: Option<Uuid>,
    pub key: String,
    pub value: String,
    pub note: String,
    pub creation_date: String,
    pub revision_date: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretIdentifier {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub key: String,
    pub projects: Vec<IdentifierProject>,
}

#[derive(Debug, Serialize)]
pub struct IdentifierProject {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSecretRequest {
    pub key: String,
    pub value: String,
    #[serde(default)]
    pub note: String,
    #[serde(alias = "project_ids")]
    pub project_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSecretRequest {
    pub key: String,
    pub value: String,
    #[serde(default)]
    pub note: String,
    pub project_ids: Option<Vec<Uuid>>,
    #[serde(default)]
    pub value_changed: bool,
}

#[derive(Debug, Deserialize)]
pub struct GetByIdsRequest {
    pub ids: Vec<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct DataResponse<T> {
    pub data: T,
}

#[derive(Debug, Serialize)]
pub struct SecretsListResponse {
    pub secrets: Vec<SecretIdentifier>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResponse {
    pub has_changes: bool,
    pub secrets: Option<DataResponse<Vec<SdkSecret>>>,
}

#[derive(Debug, Serialize)]
pub struct DeleteResult {
    pub id: Uuid,
    pub error: Option<String>,
}

impl TryFrom<project::Model> for SdkProject {
    type Error = AppError;

    fn try_from(value: project::Model) -> Result<Self, Self::Error> {
        Ok(Self {
            id: parse_uuid(&value.id)?,
            organization_id: parse_uuid(&value.organization_id)?,
            name: value.name_cipher.ok_or(AppError::NotFound)?,
            creation_date: timestamp(value.created_at)?,
            revision_date: timestamp_nanos(value.revision_nanos)?,
        })
    }
}

impl TryFrom<secret::Model> for SdkSecret {
    type Error = AppError;

    fn try_from(value: secret::Model) -> Result<Self, Self::Error> {
        Ok(Self {
            id: parse_uuid(&value.id)?,
            organization_id: parse_uuid(&value.organization_id)?,
            project_id: value.project_id.map(|id| parse_uuid(&id)).transpose()?,
            key: value.key_cipher.ok_or(AppError::NotFound)?,
            value: value.value_cipher.ok_or(AppError::NotFound)?,
            note: value.note_cipher.unwrap_or_default(),
            creation_date: timestamp(value.created_at)?,
            revision_date: timestamp_nanos(value.revision_nanos)?,
        })
    }
}

fn parse_uuid(value: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(value).map_err(AppError::internal)
}

fn timestamp(value: i64) -> Result<String, AppError> {
    use time::format_description::well_known::Rfc3339;
    time::OffsetDateTime::from_unix_timestamp(value)
        .map_err(AppError::internal)?
        .format(&Rfc3339)
        .map_err(AppError::internal)
}

fn timestamp_nanos(value: i64) -> Result<String, AppError> {
    use time::format_description::well_known::Rfc3339;
    time::OffsetDateTime::from_unix_timestamp_nanos(i128::from(value))
        .map_err(AppError::internal)?
        .format(&Rfc3339)
        .map_err(AppError::internal)
}
