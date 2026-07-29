use rand::TryRng;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, EntityTrait, IntoActiveModel,
    ModelTrait, PaginatorTrait, QueryFilter, QueryOrder, TransactionTrait,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use uuid::Uuid;

use crate::{
    db::{
        Database,
        entities::{machine_access_token, machine_account, machine_session},
    },
    domain::now,
    error::AppError,
};

pub const SDK_ENCRYPTION_KEY: &str = "X8vbvA0bduihIDe/qrzIQQ==";
pub const UPSTREAM_CLIENT_ID: &str = "ec2c1d46-6a4b-4751-a310-af9601317f2d";
pub const UPSTREAM_CLIENT_SECRET: &str = "C2IgxjjLF7qSshsbwe8JGcbM075YXw";
pub const SDK_SESSION_TTL_SECONDS: i64 = 60 * 60;
const MAX_ACTIVE_ACCESS_TOKENS: u64 = 100;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineAccount {
    pub id: Uuid,
    pub name: String,
    pub client_id: Uuid,
    pub last_used_at: Option<i64>,
    pub revoked_at: Option<i64>,
    pub compatibility_account: bool,
    pub created_at: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssuedMachineAccount {
    #[serde(flatten)]
    pub account: MachineAccount,
    pub access_token: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineAccessToken {
    pub id: Uuid,
    pub machine_account_id: Uuid,
    pub name: String,
    pub expires_at: Option<i64>,
    pub last_used_at: Option<i64>,
    pub revoked_at: Option<i64>,
    pub created_at: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssuedMachineAccessToken {
    #[serde(flatten)]
    pub token: MachineAccessToken,
    pub access_token: String,
}

pub struct AuthenticatedMachineCredential {
    pub account: MachineAccount,
    pub token_id: Uuid,
    pub expires_at: Option<i64>,
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
        let transaction = self.db.connection().begin().await?;
        if machine_account::Entity::find()
            .filter(machine_account::Column::ClientId.eq(UPSTREAM_CLIENT_ID))
            .one(&transaction)
            .await?
            .is_some()
        {
            return Ok(());
        }
        let timestamp = now();
        let account = machine_account::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            name: Set("Official SDK compatibility".into()),
            client_id: Set(UPSTREAM_CLIENT_ID.into()),
            created_by: Set(creator.to_string()),
            last_used_at: Set(None),
            revoked_at: Set(None),
            compatibility_account: Set(true),
            created_at: Set(timestamp),
        }
        .insert(&transaction)
        .await?;
        machine_access_token::ActiveModel {
            id: Set(UPSTREAM_CLIENT_ID.into()),
            machine_account_id: Set(account.id),
            name: Set("SDK compatibility".into()),
            secret_digest: Set(digest(UPSTREAM_CLIENT_SECRET)),
            expires_at: Set(None),
            last_used_at: Set(None),
            revoked_at: Set(None),
            created_at: Set(timestamp),
        }
        .insert(&transaction)
        .await?;
        transaction.commit().await?;
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

    pub async fn get(&self, id: Uuid) -> Result<MachineAccount, AppError> {
        machine_account::Entity::find_by_id(id.to_string())
            .one(self.db.connection())
            .await?
            .ok_or(AppError::NotFound)?
            .try_into()
    }

    pub async fn issue(&self, name: &str, creator: Uuid) -> Result<IssuedMachineAccount, AppError> {
        let name = validate_name(name)?;
        let client_id = Uuid::new_v4();
        let secret = random_secret()?;
        let timestamp = now();
        let transaction = self.db.connection().begin().await?;
        let model = machine_account::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            name: Set(name),
            client_id: Set(client_id.to_string()),
            created_by: Set(creator.to_string()),
            last_used_at: Set(None),
            revoked_at: Set(None),
            compatibility_account: Set(false),
            created_at: Set(timestamp),
        }
        .insert(&transaction)
        .await
        .map_err(|error| {
            if error.to_string().contains("UNIQUE") {
                AppError::Conflict
            } else {
                error.into()
            }
        })?;
        machine_access_token::ActiveModel {
            id: Set(client_id.to_string()),
            machine_account_id: Set(model.id.clone()),
            name: Set("Default".into()),
            secret_digest: Set(digest(&secret)),
            expires_at: Set(None),
            last_used_at: Set(None),
            revoked_at: Set(None),
            created_at: Set(timestamp),
        }
        .insert(&transaction)
        .await?;
        transaction.commit().await?;
        Ok(IssuedMachineAccount {
            account: MachineAccount::try_from(model)?,
            access_token: format!("0.{client_id}.{secret}:{SDK_ENCRYPTION_KEY}"),
        })
    }

    pub async fn authenticate(
        &self,
        access_token_id: &str,
        client_secret: &str,
    ) -> Result<AuthenticatedMachineCredential, AppError> {
        let timestamp = now();
        let candidate = digest(client_secret);
        let active_token = || {
            Condition::all()
                .add(machine_access_token::Column::SecretDigest.eq(candidate.clone()))
                .add(machine_access_token::Column::RevokedAt.is_null())
                .add(
                    Condition::any()
                        .add(machine_access_token::Column::ExpiresAt.is_null())
                        .add(machine_access_token::Column::ExpiresAt.gt(timestamp)),
                )
        };
        let token = machine_access_token::Entity::find_by_id(access_token_id)
            .filter(active_token())
            .one(self.db.connection())
            .await?;
        let (token, model) = if let Some(token) = token {
            let model = machine_account::Entity::find_by_id(token.machine_account_id.clone())
                .one(self.db.connection())
                .await?
                .ok_or(AppError::Unauthorized)?;
            (token, model)
        } else {
            // Older databases used the machine client ID in every token. Keep accepting those
            // records while all newly issued tokens use their own stable token ID.
            let model = machine_account::Entity::find()
                .filter(machine_account::Column::ClientId.eq(access_token_id))
                .one(self.db.connection())
                .await?
                .ok_or(AppError::Unauthorized)?;
            let token = machine_access_token::Entity::find()
                .filter(machine_access_token::Column::MachineAccountId.eq(model.id.clone()))
                .filter(active_token())
                .one(self.db.connection())
                .await?
                .ok_or(AppError::Unauthorized)?;
            (token, model)
        };
        if model.revoked_at.is_some() {
            return Err(AppError::Unauthorized);
        }
        if candidate
            .as_bytes()
            .ct_eq(token.secret_digest.as_bytes())
            .unwrap_u8()
            != 1
        {
            return Err(AppError::Unauthorized);
        }
        let transaction = self.db.connection().begin().await?;
        let mut active_token = token.clone().into_active_model();
        active_token.last_used_at = Set(Some(timestamp));
        active_token.update(&transaction).await?;
        let mut active_account = model.into_active_model();
        active_account.last_used_at = Set(Some(timestamp));
        let account = active_account.update(&transaction).await?;
        transaction.commit().await?;
        Ok(AuthenticatedMachineCredential {
            account: MachineAccount::try_from(account)?,
            token_id: Uuid::parse_str(&token.id).map_err(AppError::internal)?,
            expires_at: token.expires_at,
        })
    }

    pub async fn create_session(
        &self,
        credential: &AuthenticatedMachineCredential,
    ) -> Result<(String, i64), AppError> {
        let transaction = self.db.connection().begin().await?;
        machine_session::Entity::delete_many()
            .filter(machine_session::Column::ExpiresAt.lte(now()))
            .exec(&transaction)
            .await?;
        let token = random_secret()?;
        let created_at = now();
        let access_token =
            machine_access_token::Entity::find_by_id(credential.token_id.to_string())
                .one(&transaction)
                .await?
                .filter(|token| token.machine_account_id == credential.account.id.to_string())
                .filter(|token| token.revoked_at.is_none())
                .filter(|token| token.expires_at.is_none_or(|expires| expires > created_at))
                .ok_or(AppError::Unauthorized)?;
        let _account = machine_account::Entity::find_by_id(access_token.machine_account_id.clone())
            .one(&transaction)
            .await?
            .filter(|account| account.revoked_at.is_none())
            .ok_or(AppError::Unauthorized)?;
        let expires_at = access_token
            .expires_at
            .map_or(created_at + SDK_SESSION_TTL_SECONDS, |expires| {
                expires.min(created_at + SDK_SESSION_TTL_SECONDS)
            });
        machine_session::ActiveModel {
            id: Set(digest(&token)),
            machine_access_token_id: Set(credential.token_id.to_string()),
            expires_at: Set(expires_at),
            created_at: Set(created_at),
        }
        .insert(&transaction)
        .await?;
        transaction.commit().await?;
        Ok((token, expires_at))
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
        let access_token =
            machine_access_token::Entity::find_by_id(session.machine_access_token_id)
                .one(self.db.connection())
                .await?
                .ok_or(AppError::Unauthorized)?;
        if access_token.revoked_at.is_some()
            || access_token
                .expires_at
                .is_some_and(|expires| expires <= now())
        {
            return Err(AppError::Unauthorized);
        }
        let model = machine_account::Entity::find_by_id(access_token.machine_account_id)
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

    pub async fn list_access_tokens(
        &self,
        machine_id: Uuid,
    ) -> Result<Vec<MachineAccessToken>, AppError> {
        self.require_manageable(machine_id).await?;
        machine_access_token::Entity::find()
            .filter(machine_access_token::Column::MachineAccountId.eq(machine_id.to_string()))
            .order_by_desc(machine_access_token::Column::CreatedAt)
            .all(self.db.connection())
            .await?
            .into_iter()
            .map(MachineAccessToken::try_from)
            .collect()
    }

    pub async fn issue_access_token(
        &self,
        machine_id: Uuid,
        name: &str,
        expires_at: Option<i64>,
    ) -> Result<IssuedMachineAccessToken, AppError> {
        self.require_manageable(machine_id).await?;
        let name = validate_name(name)?;
        let timestamp = now();
        if expires_at.is_some_and(|expires| expires <= timestamp) {
            return Err(AppError::Validation(
                "access token expiry must be in the future".into(),
            ));
        }
        let active_token_count = machine_access_token::Entity::find()
            .filter(machine_access_token::Column::MachineAccountId.eq(machine_id.to_string()))
            .filter(machine_access_token::Column::RevokedAt.is_null())
            .count(self.db.connection())
            .await?;
        if active_token_count >= MAX_ACTIVE_ACCESS_TOKENS {
            return Err(AppError::Validation(
                "machine account has reached the active access token limit".into(),
            ));
        }
        let secret = random_secret()?;
        let model = machine_access_token::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            machine_account_id: Set(machine_id.to_string()),
            name: Set(name),
            secret_digest: Set(digest(&secret)),
            expires_at: Set(expires_at),
            last_used_at: Set(None),
            revoked_at: Set(None),
            created_at: Set(timestamp),
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
        let token = MachineAccessToken::try_from(model)?;
        Ok(IssuedMachineAccessToken {
            access_token: format!("0.{}.{}:{SDK_ENCRYPTION_KEY}", token.id, secret),
            token,
        })
    }

    pub async fn revoke_access_token(
        &self,
        machine_id: Uuid,
        token_id: Uuid,
    ) -> Result<MachineAccessToken, AppError> {
        self.require_manageable(machine_id).await?;
        let transaction = self.db.connection().begin().await?;
        let model = machine_access_token::Entity::find_by_id(token_id.to_string())
            .filter(machine_access_token::Column::MachineAccountId.eq(machine_id.to_string()))
            .one(&transaction)
            .await?
            .ok_or(AppError::NotFound)?;
        let mut active = model.into_active_model();
        active.revoked_at = Set(Some(now()));
        let model = active.update(&transaction).await?;
        machine_session::Entity::delete_many()
            .filter(machine_session::Column::MachineAccessTokenId.eq(token_id.to_string()))
            .exec(&transaction)
            .await?;
        transaction.commit().await?;
        MachineAccessToken::try_from(model)
    }

    async fn require_manageable(&self, id: Uuid) -> Result<MachineAccount, AppError> {
        let account = self.get(id).await?;
        if account.compatibility_account {
            return Err(AppError::Conflict);
        }
        Ok(account)
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
            compatibility_account: value.compatibility_account,
            created_at: value.created_at,
        })
    }
}

impl TryFrom<machine_access_token::Model> for MachineAccessToken {
    type Error = AppError;

    fn try_from(value: machine_access_token::Model) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Uuid::parse_str(&value.id).map_err(AppError::internal)?,
            machine_account_id: Uuid::parse_str(&value.machine_account_id)
                .map_err(AppError::internal)?,
            name: value.name,
            expires_at: value.expires_at,
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
