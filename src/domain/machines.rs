use rand::TryRng;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, IntoActiveModel, ModelTrait,
    QueryFilter, QueryOrder, TransactionTrait,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use uuid::Uuid;

use crate::{
    db::{
        Database,
        entities::{machine_account, machine_session},
    },
    domain::now,
    error::AppError,
};

pub const SDK_ENCRYPTION_KEY: &str = "X8vbvA0bduihIDe/qrzIQQ==";
pub const UPSTREAM_CLIENT_ID: &str = "ec2c1d46-6a4b-4751-a310-af9601317f2d";
pub const UPSTREAM_CLIENT_SECRET: &str = "C2IgxjjLF7qSshsbwe8JGcbM075YXw";
pub const SDK_SESSION_TTL_SECONDS: i64 = 60 * 60;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineAccount {
    pub id: Uuid,
    pub name: String,
    pub client_id: Uuid,
    pub last_used_at: Option<i64>,
    pub revoked_at: Option<i64>,
    pub created_at: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssuedMachineAccount {
    #[serde(flatten)]
    pub account: MachineAccount,
    pub access_token: String,
}

#[derive(Clone)]
pub struct MachineRepository {
    db: Database,
}

impl MachineRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn ensure_compatibility_account(&self, creator: Uuid) -> Result<(), AppError> {
        if machine_account::Entity::find()
            .filter(machine_account::Column::ClientId.eq(UPSTREAM_CLIENT_ID))
            .one(self.db.connection())
            .await?
            .is_some()
        {
            return Ok(());
        }
        machine_account::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            name: Set("Official SDK compatibility".into()),
            client_id: Set(UPSTREAM_CLIENT_ID.into()),
            client_secret_digest: Set(digest(UPSTREAM_CLIENT_SECRET)),
            created_by: Set(creator.to_string()),
            last_used_at: Set(None),
            revoked_at: Set(None),
            created_at: Set(now()),
        }
        .insert(self.db.connection())
        .await?;
        Ok(())
    }

    pub async fn list(&self) -> Result<Vec<MachineAccount>, AppError> {
        machine_account::Entity::find()
            .order_by_asc(machine_account::Column::Name)
            .all(self.db.connection())
            .await?
            .into_iter()
            .map(MachineAccount::try_from)
            .collect()
    }

    pub async fn issue(&self, name: &str, creator: Uuid) -> Result<IssuedMachineAccount, AppError> {
        let name = validate_name(name)?;
        let client_id = Uuid::new_v4();
        let secret = random_secret()?;
        let model = machine_account::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            name: Set(name),
            client_id: Set(client_id.to_string()),
            client_secret_digest: Set(digest(&secret)),
            created_by: Set(creator.to_string()),
            last_used_at: Set(None),
            revoked_at: Set(None),
            created_at: Set(now()),
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
        Ok(IssuedMachineAccount {
            account: MachineAccount::try_from(model)?,
            access_token: format!("0.{client_id}.{secret}:{SDK_ENCRYPTION_KEY}"),
        })
    }

    pub async fn authenticate(
        &self,
        client_id: &str,
        client_secret: &str,
    ) -> Result<MachineAccount, AppError> {
        let model = machine_account::Entity::find()
            .filter(machine_account::Column::ClientId.eq(client_id))
            .one(self.db.connection())
            .await?
            .ok_or(AppError::Unauthorized)?;
        if model.revoked_at.is_some()
            || digest(client_secret)
                .as_bytes()
                .ct_eq(model.client_secret_digest.as_bytes())
                .unwrap_u8()
                != 1
        {
            return Err(AppError::Unauthorized);
        }
        let mut active = model.into_active_model();
        active.last_used_at = Set(Some(now()));
        MachineAccount::try_from(active.update(self.db.connection()).await?)
    }

    pub async fn create_session(&self, machine_id: Uuid) -> Result<String, AppError> {
        let transaction = self.db.connection().begin().await?;
        machine_session::Entity::delete_many()
            .filter(machine_session::Column::ExpiresAt.lte(now()))
            .exec(&transaction)
            .await?;
        let token = random_secret()?;
        machine_session::ActiveModel {
            id: Set(digest(&token)),
            machine_account_id: Set(machine_id.to_string()),
            expires_at: Set(now() + SDK_SESSION_TTL_SECONDS),
            created_at: Set(now()),
        }
        .insert(&transaction)
        .await?;
        transaction.commit().await?;
        Ok(token)
    }

    pub async fn authenticate_session(&self, token: &str) -> Result<MachineAccount, AppError> {
        let token_digest = digest(token);
        let session = machine_session::Entity::find_by_id(&token_digest)
            .one(self.db.connection())
            .await?
            .ok_or(AppError::Unauthorized)?;
        if session.expires_at <= now() {
            machine_session::Entity::delete_by_id(token_digest)
                .exec(self.db.connection())
                .await?;
            return Err(AppError::Unauthorized);
        }
        let model = machine_account::Entity::find_by_id(session.machine_account_id)
            .one(self.db.connection())
            .await?
            .ok_or(AppError::Unauthorized)?;
        if model.revoked_at.is_some() {
            return Err(AppError::Unauthorized);
        }
        MachineAccount::try_from(model)
    }

    pub async fn set_revoked(&self, id: Uuid, revoked: bool) -> Result<MachineAccount, AppError> {
        let transaction = self.db.connection().begin().await?;
        let mut model = machine_account::Entity::find_by_id(id.to_string())
            .one(&transaction)
            .await?
            .ok_or(AppError::NotFound)?
            .into_active_model();
        model.revoked_at = Set(revoked.then_some(now()));
        let model = model.update(&transaction).await?;
        if revoked {
            machine_session::Entity::delete_many()
                .filter(machine_session::Column::MachineAccountId.eq(id.to_string()))
                .exec(&transaction)
                .await?;
        }
        transaction.commit().await?;
        MachineAccount::try_from(model)
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        let model = machine_account::Entity::find_by_id(id.to_string())
            .one(self.db.connection())
            .await?
            .ok_or(AppError::NotFound)?;
        if model.client_id == UPSTREAM_CLIENT_ID {
            return Err(AppError::Conflict);
        }
        model.delete(self.db.connection()).await?;
        Ok(())
    }
}

impl TryFrom<machine_account::Model> for MachineAccount {
    type Error = AppError;

    fn try_from(value: machine_account::Model) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Uuid::parse_str(&value.id).map_err(AppError::internal)?,
            name: value.name,
            client_id: Uuid::parse_str(&value.client_id).map_err(AppError::internal)?,
            last_used_at: value.last_used_at,
            revoked_at: value.revoked_at,
            created_at: value.created_at,
        })
    }
}

fn validate_name(value: &str) -> Result<String, AppError> {
    let value = value.trim();
    if !(1..=128).contains(&value.chars().count()) || value.chars().any(char::is_control) {
        return Err(AppError::Validation(
            "machine account name is invalid".into(),
        ));
    }
    Ok(value.into())
}

fn random_secret() -> Result<String, AppError> {
    let mut bytes = [0_u8; 22];
    rand::rng()
        .try_fill_bytes(&mut bytes)
        .map_err(AppError::internal)?;
    use base64::Engine as _;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
}

fn digest(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}
