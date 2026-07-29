use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter,
    QueryOrder, TransactionTrait,
};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    db::{
        Database,
        entities::{group, group_member, user},
    },
    domain::now,
    error::AppError,
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub id: Uuid,
    pub name: String,
    pub member_ids: Vec<Uuid>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone)]
pub struct GroupRepository {
    db: Database,
}

impl GroupRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn list(&self) -> Result<Vec<Group>, AppError> {
        let groups = group::Entity::find()
            .order_by_asc(group::Column::Name)
            .all(self.db.connection())
            .await?;
        let memberships = group_member::Entity::find()
            .all(self.db.connection())
            .await?;
        groups
            .into_iter()
            .map(|model| {
                let member_ids = memberships
                    .iter()
                    .filter(|membership| membership.group_id == model.id)
                    .map(|membership| {
                        Uuid::parse_str(&membership.user_id).map_err(AppError::internal)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Group {
                    id: Uuid::parse_str(&model.id).map_err(AppError::internal)?,
                    name: model.name,
                    member_ids,
                    created_at: model.created_at,
                    updated_at: model.updated_at,
                })
            })
            .collect()
    }

    pub async fn get(&self, id: Uuid) -> Result<Group, AppError> {
        self.list()
            .await?
            .into_iter()
            .find(|group| group.id == id)
            .ok_or(AppError::NotFound)
    }

    pub async fn create(&self, name: &str) -> Result<Group, AppError> {
        let name = validate_name(name)?;
        let timestamp = now();
        let model = group::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            name: Set(name),
            created_at: Set(timestamp),
            updated_at: Set(timestamp),
        }
        .insert(self.db.connection())
        .await
        .map_err(map_unique)?;
        Ok(Group {
            id: Uuid::parse_str(&model.id).map_err(AppError::internal)?,
            name: model.name,
            member_ids: Vec::new(),
            created_at: model.created_at,
            updated_at: model.updated_at,
        })
    }

    pub async fn update(&self, id: Uuid, name: &str) -> Result<Group, AppError> {
        let mut model = group::Entity::find_by_id(id.to_string())
            .one(self.db.connection())
            .await?
            .ok_or(AppError::NotFound)?
            .into_active_model();
        model.name = Set(validate_name(name)?);
        model.updated_at = Set(now());
        model
            .update(self.db.connection())
            .await
            .map_err(map_unique)?;
        self.get(id).await
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        let result = group::Entity::delete_by_id(id.to_string())
            .exec(self.db.connection())
            .await?;
        if result.rows_affected == 0 {
            return Err(AppError::NotFound);
        }
        Ok(())
    }

    pub async fn replace_members(&self, id: Uuid, member_ids: &[Uuid]) -> Result<Group, AppError> {
        let transaction = self.db.connection().begin().await?;
        let model = group::Entity::find_by_id(id.to_string())
            .one(&transaction)
            .await?
            .ok_or(AppError::NotFound)?;
        let unique_ids = unique_ids(member_ids);
        let existing = user::Entity::find()
            .filter(user::Column::Id.is_in(unique_ids.iter().map(Uuid::to_string)))
            .all(&transaction)
            .await?;
        if existing.len() != unique_ids.len() {
            return Err(AppError::Validation(
                "group contains an unknown user".into(),
            ));
        }
        group_member::Entity::delete_many()
            .filter(group_member::Column::GroupId.eq(id.to_string()))
            .exec(&transaction)
            .await?;
        for user_id in unique_ids {
            group_member::ActiveModel {
                group_id: Set(id.to_string()),
                user_id: Set(user_id.to_string()),
                created_at: Set(now()),
            }
            .insert(&transaction)
            .await?;
        }
        let mut active = model.into_active_model();
        active.updated_at = Set(now());
        active.update(&transaction).await?;
        transaction.commit().await?;
        self.get(id).await
    }
}

fn validate_name(value: &str) -> Result<String, AppError> {
    let value = value.trim();
    if !(1..=128).contains(&value.chars().count()) || value.chars().any(char::is_control) {
        return Err(AppError::Validation("group name is invalid".into()));
    }
    Ok(value.to_owned())
}

fn unique_ids(values: &[Uuid]) -> Vec<Uuid> {
    let mut values = values.to_vec();
    values.sort_unstable();
    values.dedup();
    values
}

fn map_unique(error: sea_orm::DbErr) -> AppError {
    if error.to_string().contains("UNIQUE") {
        AppError::Conflict
    } else {
        error.into()
    }
}
