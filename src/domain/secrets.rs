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
    domain::{access::Permission, next_sdk_revision, now},
    error::AppError,
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebSecret {
    pub id: Uuid,
    pub project_id: Option<Uuid>,
    pub key: String,
    pub value: Option<String>,
    pub note: String,
    pub sdk_encrypted: bool,
    pub deleted_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    pub permissions: Permission,
}

#[derive(Clone)]
pub struct SecretRepository {
    db: Database,
}

impl SecretRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn list(
        &self,
        include_deleted: bool,
        project_id: Option<Uuid>,
    ) -> Result<Vec<secret::Model>, AppError> {
        let mut query = secret::Entity::find().order_by_desc(secret::Column::UpdatedAt);
        if !include_deleted {
            query = query.filter(secret::Column::DeletedAt.is_null());
        }
        if let Some(project_id) = project_id {
            query = query.filter(secret::Column::ProjectId.eq(project_id.to_string()));
        }
        Ok(query.all(self.db.connection()).await?)
    }

    pub async fn get(&self, id: Uuid) -> Result<secret::Model, AppError> {
        secret::Entity::find_by_id(id.to_string())
            .one(self.db.connection())
            .await?
            .ok_or(AppError::NotFound)
    }

    pub async fn get_many(&self, ids: &[Uuid]) -> Result<Vec<secret::Model>, AppError> {
        Ok(secret::Entity::find()
            .filter(secret::Column::Id.is_in(ids.iter().map(Uuid::to_string)))
            .filter(secret::Column::DeletedAt.is_null())
            .all(self.db.connection())
            .await?)
    }

    pub async fn create_plain(
        &self,
        key: &str,
        value: &str,
        note: &str,
        project_id: impl Into<Option<Uuid>>,
    ) -> Result<WebSecret, AppError> {
        validate_plain(key, value, note)?;
        let project_id = project_id.into();
        let timestamp = now();
        let transaction = self.db.connection().begin().await?;
        if let Some(project_id) = project_id {
            require_active_project(&transaction, project_id).await?;
        }
        let revision_nanos = next_sdk_revision(&transaction).await?;
        let model = secret::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            project_id: Set(project_id.map(|id| id.to_string())),
            key_cipher: Set(None),
            value_cipher: Set(None),
            note_cipher: Set(None),
            key_plain: Set(Some(key.trim().into())),
            value_plain: Set(Some(value.into())),
            note_plain: Set(Some(note.into())),
            deleted_at: Set(None),
            created_at: Set(timestamp),
            updated_at: Set(timestamp),
            revision_nanos: Set(revision_nanos),
        }
        .insert(&transaction)
        .await?;
        transaction.commit().await?;
        WebSecret::try_from(model)
    }

    pub async fn create_cipher(
        &self,
        key: String,
        value: String,
        note: String,
        project_id: Uuid,
    ) -> Result<secret::Model, AppError> {
        validate_cipher(&key, &value, &note)?;
        let timestamp = now();
        let transaction = self.db.connection().begin().await?;
        require_active_project(&transaction, project_id).await?;
        let revision_nanos = next_sdk_revision(&transaction).await?;
        let model = secret::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            project_id: Set(Some(project_id.to_string())),
            key_cipher: Set(Some(key)),
            value_cipher: Set(Some(value)),
            note_cipher: Set(Some(note)),
            key_plain: Set(None),
            value_plain: Set(None),
            note_plain: Set(None),
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

    pub async fn update_plain(
        &self,
        id: Uuid,
        key: &str,
        value: &str,
        note: &str,
        project_id: impl Into<Option<Uuid>>,
    ) -> Result<WebSecret, AppError> {
        validate_plain(key, value, note)?;
        let project_id = project_id.into();
        let transaction = self.db.connection().begin().await?;
        if let Some(project_id) = project_id {
            require_active_project(&transaction, project_id).await?;
        }
        let model = secret::Entity::find_by_id(id.to_string())
            .one(&transaction)
            .await?
            .ok_or(AppError::NotFound)?;
        let revision_nanos = next_sdk_revision(&transaction).await?;
        let mut active = model.into_active_model();
        active.key_cipher = Set(None);
        active.value_cipher = Set(None);
        active.note_cipher = Set(None);
        active.key_plain = Set(Some(key.trim().into()));
        active.value_plain = Set(Some(value.into()));
        active.note_plain = Set(Some(note.into()));
        active.project_id = Set(project_id.map(|id| id.to_string()));
        active.updated_at = Set(now());
        active.revision_nanos = Set(revision_nanos);
        let model = active.update(&transaction).await?;
        transaction.commit().await?;
        WebSecret::try_from(model)
    }

    pub async fn update_cipher(
        &self,
        id: Uuid,
        key: String,
        value: String,
        note: String,
        project_id: Uuid,
    ) -> Result<secret::Model, AppError> {
        validate_cipher(&key, &value, &note)?;
        let transaction = self.db.connection().begin().await?;
        require_active_project(&transaction, project_id).await?;
        let model = secret::Entity::find_by_id(id.to_string())
            .one(&transaction)
            .await?
            .ok_or(AppError::NotFound)?;
        let revision_nanos = next_sdk_revision(&transaction).await?;
        let mut active = model.into_active_model();
        active.key_cipher = Set(Some(key));
        active.value_cipher = Set(Some(value));
        active.note_cipher = Set(Some(note));
        active.key_plain = Set(None);
        active.value_plain = Set(None);
        active.note_plain = Set(None);
        active.project_id = Set(Some(project_id.to_string()));
        active.updated_at = Set(now());
        active.revision_nanos = Set(revision_nanos);
        let model = active.update(&transaction).await?;
        transaction.commit().await?;
        Ok(model)
    }

    pub async fn set_deleted(&self, ids: &[Uuid], deleted: bool) -> Result<(), AppError> {
        if ids.is_empty() {
            return Ok(());
        }
        let transaction = self.db.connection().begin().await?;
        let models = secret::Entity::find()
            .filter(secret::Column::Id.is_in(ids.iter().map(Uuid::to_string)))
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
        if secret::Entity::find_by_id(id.to_string())
            .one(&transaction)
            .await?
            .is_none()
        {
            return Err(AppError::NotFound);
        }
        next_sdk_revision(&transaction).await?;
        secret::Entity::delete_by_id(id.to_string())
            .exec(&transaction)
            .await?;
        transaction.commit().await?;
        Ok(())
    }
}

impl TryFrom<secret::Model> for WebSecret {
    type Error = AppError;

    fn try_from(value: secret::Model) -> Result<Self, Self::Error> {
        let sdk_encrypted = value.key_cipher.is_some();
        Ok(Self {
            id: Uuid::parse_str(&value.id).map_err(AppError::internal)?,
            project_id: value
                .project_id
                .map(|id| Uuid::parse_str(&id).map_err(AppError::internal))
                .transpose()?,
            key: value
                .key_plain
                .unwrap_or_else(|| "Encrypted SDK secret".into()),
            value: value.value_plain,
            note: value.note_plain.unwrap_or_default(),
            sdk_encrypted,
            deleted_at: value.deleted_at,
            created_at: value.created_at,
            updated_at: value.updated_at,
            permissions: Permission::FULL,
        })
    }
}

async fn require_active_project(
    connection: &impl sea_orm::ConnectionTrait,
    project_id: Uuid,
) -> Result<(), AppError> {
    project::Entity::find_by_id(project_id.to_string())
        .filter(project::Column::DeletedAt.is_null())
        .one(connection)
        .await?
        .map(|_| ())
        .ok_or_else(|| AppError::Validation("secret project does not exist or is deleted".into()))
}

fn validate_plain(key: &str, value: &str, note: &str) -> Result<(), AppError> {
    if key.trim().is_empty()
        || key.chars().count() > 500
        || value.len() > 1024 * 1024
        || note.len() > 64 * 1024
        || key.chars().any(char::is_control)
    {
        return Err(AppError::Validation("secret fields are invalid".into()));
    }
    Ok(())
}

fn validate_cipher(key: &str, value: &str, note: &str) -> Result<(), AppError> {
    if key.is_empty()
        || key.len() > 32 * 1024
        || value.is_empty()
        || value.len() > 2 * 1024 * 1024
        || note.len() > 128 * 1024
    {
        return Err(AppError::Validation(
            "encrypted secret fields are invalid".into(),
        ));
    }
    Ok(())
}
