use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
};
use rand::TryRng;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseBackend, EntityTrait,
    IntoActiveModel, QueryFilter, QueryOrder, Statement,
};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    config::{BootstrapAdmin, validate_password, validate_username},
    db::{Database, entities::user},
    domain::now,
    error::AppError,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Admin,
    User,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::User => "user",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicUser {
    pub id: Uuid,
    pub username: String,
    pub display_name: String,
    pub role: Role,
    pub disabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_login_at: Option<i64>,
}

#[derive(Clone)]
pub struct UserRepository {
    db: Database,
}

impl UserRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn bootstrap(&self, admin: Option<&BootstrapAdmin>) -> Result<(), AppError> {
        if user::Entity::find()
            .one(self.db.connection())
            .await?
            .is_some()
        {
            return Ok(());
        }
        let Some(admin) = admin else {
            return Err(AppError::Validation(
                "bootstrap administrator environment variables are required for an empty database"
                    .into(),
            ));
        };
        self.create(
            &admin.username,
            &admin.username,
            Role::Admin,
            admin.password.expose_secret(),
        )
        .await?;
        Ok(())
    }

    pub async fn list(&self) -> Result<Vec<PublicUser>, AppError> {
        user::Entity::find()
            .order_by_asc(user::Column::Username)
            .all(self.db.connection())
            .await?
            .into_iter()
            .map(PublicUser::try_from)
            .collect()
    }

    pub async fn get(&self, id: Uuid) -> Result<PublicUser, AppError> {
        let model = user::Entity::find_by_id(id.to_string())
            .one(self.db.connection())
            .await?
            .ok_or(AppError::Unauthorized)?;
        PublicUser::try_from(model)
    }

    pub async fn find_login(&self, username: &str) -> Result<user::Model, AppError> {
        user::Entity::find()
            .filter(user::Column::Username.eq(username.trim()))
            .one(self.db.connection())
            .await?
            .ok_or(AppError::Unauthorized)
    }

    pub async fn create(
        &self,
        username: &str,
        display_name: &str,
        role: Role,
        password: &str,
    ) -> Result<PublicUser, AppError> {
        let username =
            validate_username(username).map_err(|error| AppError::Validation(error.to_string()))?;
        let display_name = validate_display_name(display_name)?;
        validate_password(password).map_err(|error| AppError::Validation(error.to_string()))?;
        let hash = hash_password(password)?;
        let timestamp = now();
        let model = user::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            username: Set(username.to_owned()),
            display_name: Set(display_name),
            role: Set(role.as_str().into()),
            password_hash: Set(hash),
            disabled: Set(false),
            created_at: Set(timestamp),
            updated_at: Set(timestamp),
            last_login_at: Set(None),
        }
        .insert(self.db.connection())
        .await
        .map_err(|error| {
            if error.to_string().contains("UNIQUE") {
                AppError::Conflict
            } else {
                error.into()
            }
        })?;
        PublicUser::try_from(model)
    }

    pub async fn update(
        &self,
        id: Uuid,
        display_name: &str,
        role: Role,
        disabled: bool,
    ) -> Result<PublicUser, AppError> {
        let display_name = validate_display_name(display_name)?;
        let role = role.as_str();
        let result = self
            .db
            .connection()
            .execute_raw(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                r#"
                UPDATE users
                SET display_name = ?, role = ?, disabled = ?, updated_at = ?
                WHERE id = ?
                  AND (
                    role <> 'admin'
                    OR disabled = 1
                    OR (? = 'admin' AND ? = 0)
                    OR (SELECT COUNT(*) FROM users WHERE role = 'admin' AND disabled = 0) > 1
                  )
                "#,
                [
                    display_name.into(),
                    role.into(),
                    disabled.into(),
                    now().into(),
                    id.to_string().into(),
                    role.into(),
                    disabled.into(),
                ],
            ))
            .await?;
        if result.rows_affected() == 0 {
            return if user::Entity::find_by_id(id.to_string())
                .one(self.db.connection())
                .await?
                .is_some()
            {
                Err(AppError::Conflict)
            } else {
                Err(AppError::NotFound)
            };
        }
        self.get(id).await
    }

    pub async fn reset_password(&self, id: Uuid, password: &str) -> Result<(), AppError> {
        validate_password(password).map_err(|error| AppError::Validation(error.to_string()))?;
        let mut model = user::Entity::find_by_id(id.to_string())
            .one(self.db.connection())
            .await?
            .ok_or(AppError::NotFound)?
            .into_active_model();
        model.password_hash = Set(hash_password(password)?);
        model.updated_at = Set(now());
        model.update(self.db.connection()).await?;
        Ok(())
    }

    pub async fn record_login(&self, model: user::Model) -> Result<PublicUser, AppError> {
        let mut active = model.into_active_model();
        active.last_login_at = Set(Some(now()));
        PublicUser::try_from(active.update(self.db.connection()).await?)
    }
}

impl TryFrom<user::Model> for PublicUser {
    type Error = AppError;

    fn try_from(value: user::Model) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Uuid::parse_str(&value.id).map_err(AppError::internal)?,
            username: value.username,
            display_name: value.display_name,
            role: match value.role.as_str() {
                "admin" => Role::Admin,
                "user" => Role::User,
                _ => return Err(AppError::internal(anyhow::anyhow!("invalid stored role"))),
            },
            disabled: value.disabled,
            created_at: value.created_at,
            updated_at: value.updated_at,
            last_login_at: value.last_login_at,
        })
    }
}

pub fn hash_password(password: &str) -> Result<String, AppError> {
    let mut salt_bytes = [0_u8; 16];
    rand::rng()
        .try_fill_bytes(&mut salt_bytes)
        .map_err(AppError::internal)?;
    let salt = SaltString::encode_b64(&salt_bytes).map_err(AppError::internal)?;
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(AppError::internal)
}

pub fn verify_password(hash: &str, password: &str) -> bool {
    PasswordHash::new(hash).ok().is_some_and(|hash| {
        Argon2::default()
            .verify_password(password.as_bytes(), &hash)
            .is_ok()
    })
}

fn validate_display_name(value: &str) -> Result<String, AppError> {
    let value = value.trim();
    if !(1..=128).contains(&value.chars().count()) || value.chars().any(char::is_control) {
        return Err(AppError::Validation("display name is invalid".into()));
    }
    Ok(value.to_owned())
}
