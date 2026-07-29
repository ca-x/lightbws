use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter,
    QueryOrder, TransactionTrait,
};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    db::{
        Database,
        entities::{project, secret},
    },
    domain::{ORGANIZATION_ID, access::Permission, next_sdk_revision, now},
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
    pub permissions: Permission,
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
        let transaction = self.db.connection().begin().await?;
        let revision_nanos = next_sdk_revision(&transaction).await?;
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
        .insert(&transaction)
        .await?;
        transaction.commit().await?;
        WebProject::try_from(model)
    }

    pub async fn create_cipher(&self, name: String) -> Result<project::Model, AppError> {
        validate_cipher(&name)?;
        let timestamp = now();
        let transaction = self.db.connection().begin().await?;
        let revision_nanos = next_sdk_revision(&transaction).await?;
        let model = project::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            organization_id: Set(ORGANIZATION_ID.into()),
            name_cipher: Set(Some(name)),
            name_plain: Set(None),
            deleted_at: Set(None),
            created_at: Set(timestamp),
            updated_at: Set(timestamp),
            revision_nanos: Set(revision_nanos),
        }
        .insert(&transaction)
        .await?;
        transaction.commit().await?;
        Ok(model)
    }

    pub async fn update_plain(&self, id: Uuid, name: &str) -> Result<WebProject, AppError> {
        let transaction = self.db.connection().begin().await?;
        let model = project::Entity::find_by_id(id.to_string())
            .one(&transaction)
            .await?
            .ok_or(AppError::NotFound)?;
        let revision_nanos = next_sdk_revision(&transaction).await?;
        let mut model = model.into_active_model();
        model.name_cipher = Set(None);
        model.name_plain = Set(Some(validate_name(name)?));
        model.updated_at = Set(now());
        model.revision_nanos = Set(revision_nanos);
        let model = model.update(&transaction).await?;
        transaction.commit().await?;
        WebProject::try_from(model)
    }

    pub async fn update_cipher(&self, id: Uuid, name: String) -> Result<project::Model, AppError> {
        validate_cipher(&name)?;
        let transaction = self.db.connection().begin().await?;
        let model = project::Entity::find_by_id(id.to_string())
            .one(&transaction)
            .await?
            .ok_or(AppError::NotFound)?;
        let revision_nanos = next_sdk_revision(&transaction).await?;
        let mut model = model.into_active_model();
        model.name_cipher = Set(Some(name));
        model.name_plain = Set(None);
        model.updated_at = Set(now());
        model.revision_nanos = Set(revision_nanos);
        let model = model.update(&transaction).await?;
        transaction.commit().await?;
        Ok(model)
    }

    pub async fn set_deleted(&self, ids: &[Uuid], deleted: bool) -> Result<(), AppError> {
        if ids.is_empty() {
            return Ok(());
        }
        let transaction = self.db.connection().begin().await?;
        let models = project::Entity::find()
            .filter(project::Column::Id.is_in(ids.iter().map(Uuid::to_string)))
            .all(&transaction)
            .await?;
        if models.is_empty() {
            transaction.commit().await?;
            return Ok(());
        }
        let timestamp = now();
        let revision_nanos = next_sdk_revision(&transaction).await?;
        for model in models {
            let mut active = model.into_active_model();
            active.deleted_at = Set(deleted.then_some(timestamp));
            active.updated_at = Set(timestamp);
            active.revision_nanos = Set(revision_nanos);
            active.update(&transaction).await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    pub async fn purge(&self, id: Uuid) -> Result<(), AppError> {
        let transaction = self.db.connection().begin().await?;
        secret::Entity::delete_many()
            .filter(secret::Column::ProjectId.eq(id.to_string()))
            .filter(secret::Column::KeyCipher.is_not_null())
            .exec(&transaction)
            .await?;
        let result = project::Entity::delete_by_id(id.to_string())
            .exec(&transaction)
            .await?;
        if result.rows_affected == 0 {
            return Err(AppError::NotFound);
        }
        next_sdk_revision(&transaction).await?;
        transaction.commit().await?;
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
            permissions: Permission::FULL,
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
