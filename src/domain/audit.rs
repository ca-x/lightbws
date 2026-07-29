use std::time::Duration;

use sea_orm::{
    ColumnTrait, Condition, ConnectionTrait, DatabaseBackend, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Statement, TransactionTrait, sea_query::Expr,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    db::{
        Database,
        entities::{audit_event, audit_setting, machine_access_token},
    },
    domain::now,
    error::AppError,
};

const AUDIT_SETTINGS_ID: i32 = 1;
const CLEANUP_INTERVAL_SECONDS: i64 = 60 * 60;

#[derive(Clone, Copy)]
pub enum AuditActor {
    User(Uuid),
    Machine(Uuid),
    System,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEvent {
    pub id: Uuid,
    pub actor_kind: String,
    pub actor_id: Option<Uuid>,
    pub action: String,
    pub resource_kind: String,
    pub resource_id: Option<Uuid>,
    pub outcome: String,
    pub created_at: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditSettings {
    pub enabled: bool,
    pub auto_cleanup_enabled: bool,
    pub retention_days: u16,
    pub last_cleanup_at: Option<i64>,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateAuditSettings {
    pub enabled: bool,
    pub auto_cleanup_enabled: bool,
    pub retention_days: u16,
}

#[derive(Clone)]
pub struct AuditRepository {
    db: Database,
}

impl AuditRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn record(
        &self,
        actor: AuditActor,
        action: &str,
        resource_kind: &str,
        resource_id: Option<Uuid>,
        outcome: &str,
    ) -> Result<(), AppError> {
        let (actor_kind, actor_id) = match actor {
            AuditActor::User(id) => ("user", Some(id.to_string())),
            AuditActor::Machine(id) => ("machine", Some(id.to_string())),
            AuditActor::System => ("system", None),
        };
        let action = validate_label(action, "audit action", 100)?;
        let resource_kind = validate_label(resource_kind, "resource kind", 64)?;
        let outcome = match outcome {
            "allowed" | "denied" | "changed" => outcome,
            _ => return Err(AppError::Validation("audit outcome is invalid".into())),
        };
        self.db
            .connection()
            .execute_raw(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                r#"
                INSERT INTO audit_events (
                    id, actor_kind, actor_id, action, resource_kind, resource_id, outcome, created_at
                )
                SELECT ?, ?, ?, ?, ?, ?, ?, ?
                FROM audit_settings
                WHERE id = 1 AND enabled = 1
                "#,
                [
                    Uuid::new_v4().to_string().into(),
                    actor_kind.into(),
                    actor_id.into(),
                    action.into(),
                    resource_kind.into(),
                    resource_id.map(|id| id.to_string()).into(),
                    outcome.into(),
                    now().into(),
                ],
            ))
            .await?;
        Ok(())
    }

    pub async fn list(&self) -> Result<Vec<AuditEvent>, AppError> {
        audit_event::Entity::find()
            .order_by_desc(audit_event::Column::CreatedAt)
            .order_by_desc(audit_event::Column::Id)
            .limit(500)
            .all(self.db.connection())
            .await?
            .into_iter()
            .map(AuditEvent::try_from)
            .collect()
    }

    pub async fn list_machine(
        &self,
        machine_id: Uuid,
        from: Option<i64>,
        to: Option<i64>,
    ) -> Result<Vec<AuditEvent>, AppError> {
        if from.zip(to).is_some_and(|(from, to)| from > to) {
            return Err(AppError::Validation("audit time range is invalid".into()));
        }
        let id = machine_id.to_string();
        let token_ids = machine_access_token::Entity::find()
            .select_only()
            .column(machine_access_token::Column::Id)
            .filter(machine_access_token::Column::MachineAccountId.eq(id.clone()))
            .into_tuple::<String>()
            .all(self.db.connection())
            .await?;
        let mut related = Condition::any()
            .add(
                Condition::all()
                    .add(audit_event::Column::ActorKind.eq("machine"))
                    .add(audit_event::Column::ActorId.eq(id.clone())),
            )
            .add(
                Condition::all()
                    .add(audit_event::Column::ResourceKind.eq("machine"))
                    .add(audit_event::Column::ResourceId.eq(id)),
            );
        if !token_ids.is_empty() {
            related = related.add(
                Condition::all()
                    .add(audit_event::Column::ResourceKind.eq("machine_token"))
                    .add(audit_event::Column::ResourceId.is_in(token_ids)),
            );
        }
        let mut query = audit_event::Entity::find().filter(related);
        if let Some(from) = from {
            query = query.filter(audit_event::Column::CreatedAt.gte(from));
        }
        if let Some(to) = to {
            query = query.filter(audit_event::Column::CreatedAt.lte(to));
        }
        query
            .order_by_desc(audit_event::Column::CreatedAt)
            .order_by_desc(audit_event::Column::Id)
            .limit(500)
            .all(self.db.connection())
            .await?
            .into_iter()
            .map(AuditEvent::try_from)
            .collect()
    }

    pub async fn settings(&self) -> Result<AuditSettings, AppError> {
        audit_setting::Entity::find_by_id(AUDIT_SETTINGS_ID)
            .one(self.db.connection())
            .await?
            .ok_or_else(|| AppError::internal(anyhow::anyhow!("audit settings are missing")))?
            .try_into()
    }

    pub async fn update_settings(
        &self,
        input: UpdateAuditSettings,
    ) -> Result<AuditSettings, AppError> {
        if !(1..=3650).contains(&input.retention_days) {
            return Err(AppError::Validation(
                "audit retention must be between 1 and 3650 days".into(),
            ));
        }
        audit_setting::Entity::update_many()
            .col_expr(audit_setting::Column::Enabled, Expr::value(input.enabled))
            .col_expr(
                audit_setting::Column::AutoCleanupEnabled,
                Expr::value(input.auto_cleanup_enabled),
            )
            .col_expr(
                audit_setting::Column::RetentionDays,
                Expr::value(i32::from(input.retention_days)),
            )
            .col_expr(audit_setting::Column::UpdatedAt, Expr::value(now()))
            .filter(audit_setting::Column::Id.eq(AUDIT_SETTINGS_ID))
            .exec(self.db.connection())
            .await?;
        self.settings().await
    }

    pub async fn clear(&self) -> Result<u64, AppError> {
        self.delete_events(None).await
    }

    pub async fn cleanup_if_due(&self) -> Result<u64, AppError> {
        let settings = audit_setting::Entity::find_by_id(AUDIT_SETTINGS_ID)
            .one(self.db.connection())
            .await?
            .ok_or_else(|| AppError::internal(anyhow::anyhow!("audit settings are missing")))?;
        let timestamp = now();
        if !settings.auto_cleanup_enabled
            || settings
                .last_cleanup_at
                .is_some_and(|last| last > timestamp - CLEANUP_INTERVAL_SECONDS)
        {
            return Ok(0);
        }
        let cutoff = timestamp - i64::from(settings.retention_days) * 24 * 60 * 60;
        self.delete_events(Some(cutoff)).await
    }

    async fn delete_events(&self, before: Option<i64>) -> Result<u64, AppError> {
        let transaction = self.db.connection().begin().await?;
        audit_setting::Entity::update_many()
            .col_expr(audit_setting::Column::CleanupAuthorized, Expr::value(true))
            .filter(audit_setting::Column::Id.eq(AUDIT_SETTINGS_ID))
            .exec(&transaction)
            .await?;
        let mut delete = audit_event::Entity::delete_many();
        if let Some(before) = before {
            delete = delete.filter(audit_event::Column::CreatedAt.lt(before));
        }
        let result = delete.exec(&transaction).await?;
        audit_setting::Entity::update_many()
            .col_expr(audit_setting::Column::CleanupAuthorized, Expr::value(false))
            .col_expr(
                audit_setting::Column::LastCleanupAt,
                Expr::value(Some(now())),
            )
            .filter(audit_setting::Column::Id.eq(AUDIT_SETTINGS_ID))
            .exec(&transaction)
            .await?;
        transaction.commit().await?;
        Ok(result.rows_affected)
    }
}

pub async fn scheduler(db: Database) {
    let mut interval = tokio::time::interval(Duration::from_secs(60 * 60));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        if let Err(error) = AuditRepository::new(db.clone()).cleanup_if_due().await {
            tracing::warn!(error = %error, "audit cleanup failed");
        }
    }
}

impl TryFrom<audit_setting::Model> for AuditSettings {
    type Error = AppError;

    fn try_from(value: audit_setting::Model) -> Result<Self, Self::Error> {
        Ok(Self {
            enabled: value.enabled,
            auto_cleanup_enabled: value.auto_cleanup_enabled,
            retention_days: u16::try_from(value.retention_days).map_err(AppError::internal)?,
            last_cleanup_at: value.last_cleanup_at,
            updated_at: value.updated_at,
        })
    }
}

impl TryFrom<audit_event::Model> for AuditEvent {
    type Error = AppError;

    fn try_from(event: audit_event::Model) -> Result<Self, Self::Error> {
        Ok(Self {
            id: Uuid::parse_str(&event.id).map_err(AppError::internal)?,
            actor_kind: event.actor_kind,
            actor_id: event
                .actor_id
                .map(|id| Uuid::parse_str(&id).map_err(AppError::internal))
                .transpose()?,
            action: event.action,
            resource_kind: event.resource_kind,
            resource_id: event
                .resource_id
                .map(|id| Uuid::parse_str(&id).map_err(AppError::internal))
                .transpose()?,
            outcome: event.outcome,
            created_at: event.created_at,
        })
    }
}

fn validate_label(value: &str, label: &str, max_len: usize) -> Result<String, AppError> {
    if value.is_empty()
        || value.len() > max_len
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(AppError::Validation(format!("{label} is invalid")));
    }
    Ok(value.into())
}
