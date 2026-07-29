use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    db::entities::{project, secret},
    domain::ORGANIZATION_ID,
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
    pub projects: Vec<SdkSecretProject>,
    pub key: String,
    pub value: String,
    pub note: String,
    pub creation_date: String,
    pub revision_date: String,
}

#[derive(Debug, Serialize)]
pub struct SdkSecretProject {
    pub id: Uuid,
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
    pub access_policies_requests: Option<SecretAccessPoliciesRequests>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSecretRequest {
    pub key: String,
    pub value: String,
    #[serde(default)]
    pub note: String,
    pub project_ids: Option<Vec<Uuid>>,
    pub access_policies_requests: Option<SecretAccessPoliciesRequests>,
    #[serde(default)]
    pub value_changed: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessPolicyRequest {
    pub grantee_id: Uuid,
    pub read: bool,
    pub write: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretAccessPoliciesRequests {
    pub user_access_policy_requests: Option<Vec<AccessPolicyRequest>>,
    pub group_access_policy_requests: Option<Vec<AccessPolicyRequest>>,
    pub service_account_access_policy_requests: Option<Vec<AccessPolicyRequest>>,
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
            organization_id: parse_uuid(ORGANIZATION_ID)?,
            projects: value
                .project_id
                .as_deref()
                .map(parse_uuid)
                .transpose()?
                .map(|id| vec![SdkSecretProject { id }])
                .unwrap_or_default(),
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
