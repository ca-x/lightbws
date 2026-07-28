use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter,
    QueryOrder,
};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    db::{Database, entities::project},
    domain::{ORGANIZATION_ID, now, now_nanos},
    error::AppError,
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebProject {
    pub id: Uuid,
    pub name: String,
    pub sdk_encrypted: bool,
    pub deleted_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone)]
pub struct ProjectRepository {
    db: Database,
}

impl ProjectRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn list(&self, include_deleted: bool) -> Result<Vec<project::Model>, AppError> {
        let mut query = project::Entity::find()
            .filter(project::Column::OrganizationId.eq(ORGANIZATION_ID))
            .order_by_desc(project::Column::UpdatedAt);
        if !include_deleted {
            query = query.filter(project::Column::DeletedAt.is_null());
        }
        Ok(query.all(self.db.connection()).await?)
    }

    pub async fn get(&self, id: Uuid) -> Result<project::Model, AppError> {
        project::Entity::find_by_id(id.to_string())
            .one(self.db.connection())
            .await?
            .ok_or(AppError::NotFound)
    }

    pub async fn create_plain(&self, name: &str) -> Result<WebProject, AppError> {
        let name = validate_name(name)?;
        let timestamp = now();
        let revision_nanos = now_nanos();
        let model = project::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            organization_id: Set(ORGANIZATION_ID.into()),
            name_cipher: Set(None),
            name_plain: Set(Some(name)),
            deleted_at: Set(None),
            created_at: Set(timestamp),
            updated_at: Set(timestamp),
            revision_nanos: Set(revision_nanos),
        }
        .insert(self.db.connection())
        .await?;
        WebProject::try_from(model)
    }

    pub async fn create_cipher(&self, name: String) -> Result<project::Model, AppError> {
        validate_cipher(&name)?;
        let timestamp = now();
        let revision_nanos = now_nanos();
        Ok(project::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            organization_id: Set(ORGANIZATION_ID.into()),
            name_cipher: Set(Some(name)),
            name_plain: Set(None),
            deleted_at: Set(None),
            created_at: Set(timestamp),
            updated_at: Set(timestamp),
            revision_nanos: Set(revision_nanos),
        }
        .insert(self.db.connection())
        .await?)
    }

    pub async fn update_plain(&self, id: Uuid, name: &str) -> Result<WebProject, AppError> {
        let model = self.get(id).await?;
        let revision_nanos = now_nanos().max(model.revision_nanos.saturating_add(1));
        let mut model = model.into_active_model();
        model.name_cipher = Set(None);
        model.name_plain = Set(Some(validate_name(name)?));
        model.updated_at = Set(now());
        model.revision_nanos = Set(revision_nanos);
        WebProject::try_from(model.update(self.db.connection()).await?)
    }

    pub async fn update_cipher(&self, id: Uuid, name: String) -> Result<project::Model, AppError> {
        validate_cipher(&name)?;
        let model = self.get(id).await?;
        let revision_nanos = now_nanos().max(model.revision_nanos.saturating_add(1));
        let mut model = model.into_active_model();
        model.name_cipher = Set(Some(name));
        model.name_plain = Set(None);
        model.updated_at = Set(now());
        model.revision_nanos = Set(revision_nanos);
        Ok(model.update(self.db.connection()).await?)
    }

    pub async fn set_deleted(&self, ids: &[Uuid], deleted: bool) -> Result<(), AppError> {
        let timestamp = now();
        for id in ids {
            let Some(model) = project::Entity::find_by_id(id.to_string())
                .one(self.db.connection())
                .await?
            else {
                continue;
            };
            let revision_nanos = now_nanos().max(model.revision_nanos.saturating_add(1));
            let mut active = model.into_active_model();
            active.deleted_at = Set(deleted.then_some(timestamp));
            active.updated_at = Set(timestamp);
            active.revision_nanos = Set(revision_nanos);
            active.update(self.db.connection()).await?;
        }
        Ok(())
    }

    pub async fn purge(&self, id: Uuid) -> Result<(), AppError> {
        let result = project::Entity::delete_by_id(id.to_string())
            .exec(self.db.connection())
            .await?;
        if result.rows_affected == 0 {
            return Err(AppError::NotFound);
        }
        Ok(())
    }
}

impl TryFrom<project::Model> for WebProject {
    type Error = AppError;

    fn try_from(value: project::Model) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Uuid::parse_str(&value.id).map_err(AppError::internal)?,
            name: value
                .name_plain
                .unwrap_or_else(|| "Encrypted SDK project".into()),
            sdk_encrypted: value.name_cipher.is_some(),
            deleted_at: value.deleted_at,
            created_at: value.created_at,
            updated_at: value.updated_at,
        })
    }
}

fn validate_name(value: &str) -> Result<String, AppError> {
    let value = value.trim();
    if !(1..=500).contains(&value.chars().count()) || value.chars().any(char::is_control) {
        return Err(AppError::Validation("project name is invalid".into()));
    }
    Ok(value.into())
}

fn validate_cipher(value: &str) -> Result<(), AppError> {
    if value.is_empty() || value.len() > 16_384 {
        return Err(AppError::Validation(
            "encrypted project name is invalid".into(),
        ));
    }
    Ok(())
}
