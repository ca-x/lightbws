use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseBackend, EntityTrait,
    QueryFilter, Statement, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    db::{
        Database,
        entities::{
            group, machine_account, machine_group_grant, machine_user_grant, project,
            project_group_grant, project_machine_grant, project_user_grant, secret,
            secret_group_grant, secret_machine_grant, secret_user_grant, user,
        },
    },
    domain::{machines::MachineAccount, next_sdk_revision, now, users::Role},
    error::AppError,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Permission {
    pub read: bool,
    pub write: bool,
}

impl Permission {
    pub const FULL: Self = Self {
        read: true,
        write: true,
    };

    pub fn combine(self, other: Self) -> Self {
        Self {
            read: self.read || other.read,
            write: self.write || other.write,
        }
    }

    pub fn require_read(self) -> Result<(), AppError> {
        self.read.then_some(()).ok_or(AppError::Forbidden)
    }

    pub fn require_write(self) -> Result<(), AppError> {
        self.write.then_some(()).ok_or(AppError::Forbidden)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrantInput {
    pub grantee_id: Uuid,
    pub read: bool,
    pub write: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NamedGrant {
    pub grantee_id: Uuid,
    pub name: String,
    pub read: bool,
    pub write: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessPolicyInput {
    #[serde(default)]
    pub users: Vec<GrantInput>,
    #[serde(default)]
    pub groups: Vec<GrantInput>,
    #[serde(default)]
    pub machines: Vec<GrantInput>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessPolicyView {
    pub users: Vec<NamedGrant>,
    pub groups: Vec<NamedGrant>,
    pub machines: Vec<NamedGrant>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineAccessInput {
    #[serde(default)]
    pub users: Vec<GrantInput>,
    #[serde(default)]
    pub groups: Vec<GrantInput>,
    #[serde(default)]
    pub projects: Vec<GrantInput>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineAccessView {
    pub users: Vec<NamedGrant>,
    pub groups: Vec<NamedGrant>,
    pub projects: Vec<NamedGrant>,
}

#[derive(Clone)]
pub struct AccessRepository {
    db: Database,
}

impl AccessRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn user_project(
        &self,
        user_id: Uuid,
        role: Role,
        project_id: Uuid,
    ) -> Result<Permission, AppError> {
        if role == Role::Admin {
            return Ok(Permission::FULL);
        }
        if project::Entity::find_by_id(project_id.to_string())
            .filter(project::Column::DeletedAt.is_null())
            .one(self.db.connection())
            .await?
            .is_none()
        {
            return Ok(Permission::default());
        }
        permission_query(
            self.db.connection(),
            r#"
            SELECT COALESCE(MAX(can_read), 0) AS can_read,
                   COALESCE(MAX(can_write), 0) AS can_write
            FROM (
                SELECT can_read, can_write
                FROM project_user_grants
                WHERE project_id = ? AND user_id = ?
                UNION ALL
                SELECT grant_policy.can_read, grant_policy.can_write
                FROM project_group_grants grant_policy
                JOIN group_members membership ON membership.group_id = grant_policy.group_id
                WHERE grant_policy.project_id = ? AND membership.user_id = ?
            )
            "#,
            [
                project_id.to_string().into(),
                user_id.to_string().into(),
                project_id.to_string().into(),
                user_id.to_string().into(),
            ],
        )
        .await
    }

    pub async fn machine_project(
        &self,
        machine: &MachineAccount,
        project_id: Uuid,
    ) -> Result<Permission, AppError> {
        if project::Entity::find_by_id(project_id.to_string())
            .filter(project::Column::DeletedAt.is_null())
            .one(self.db.connection())
            .await?
            .is_none()
        {
            return Ok(Permission::default());
        }
        if machine.compatibility_account {
            return Ok(Permission::FULL);
        }
        permission_query(
            self.db.connection(),
            r#"
            SELECT COALESCE(MAX(can_read), 0) AS can_read,
                   COALESCE(MAX(can_write), 0) AS can_write
            FROM project_machine_grants
            WHERE project_id = ? AND machine_account_id = ?
            "#,
            [project_id.to_string().into(), machine.id.to_string().into()],
        )
        .await
    }

    pub async fn user_secret(
        &self,
        user_id: Uuid,
        role: Role,
        model: &secret::Model,
    ) -> Result<Permission, AppError> {
        if role == Role::Admin {
            return Ok(Permission::FULL);
        }
        let project_id = Uuid::parse_str(&model.project_id).map_err(AppError::internal)?;
        if project::Entity::find_by_id(project_id.to_string())
            .filter(project::Column::DeletedAt.is_null())
            .one(self.db.connection())
            .await?
            .is_none()
        {
            return Ok(Permission::default());
        }
        let project = self.user_project(user_id, role, project_id).await?;
        let direct = permission_query(
            self.db.connection(),
            r#"
            SELECT COALESCE(MAX(can_read), 0) AS can_read,
                   COALESCE(MAX(can_write), 0) AS can_write
            FROM (
                SELECT can_read, can_write
                FROM secret_user_grants
                WHERE secret_id = ? AND user_id = ?
                UNION ALL
                SELECT grant_policy.can_read, grant_policy.can_write
                FROM secret_group_grants grant_policy
                JOIN group_members membership ON membership.group_id = grant_policy.group_id
                WHERE grant_policy.secret_id = ? AND membership.user_id = ?
            )
            "#,
            [
                model.id.clone().into(),
                user_id.to_string().into(),
                model.id.clone().into(),
                user_id.to_string().into(),
            ],
        )
        .await?;
        Ok(project.combine(direct))
    }

    pub async fn machine_secret(
        &self,
        machine: &MachineAccount,
        model: &secret::Model,
    ) -> Result<Permission, AppError> {
        let project_id = Uuid::parse_str(&model.project_id).map_err(AppError::internal)?;
        if project::Entity::find_by_id(project_id.to_string())
            .filter(project::Column::DeletedAt.is_null())
            .one(self.db.connection())
            .await?
            .is_none()
        {
            return Ok(Permission::default());
        }
        let project = self.machine_project(machine, project_id).await?;
        let direct = permission_query(
            self.db.connection(),
            r#"
            SELECT COALESCE(MAX(can_read), 0) AS can_read,
                   COALESCE(MAX(can_write), 0) AS can_write
            FROM secret_machine_grants
            WHERE secret_id = ? AND machine_account_id = ?
            "#,
            [model.id.clone().into(), machine.id.to_string().into()],
        )
        .await?;
        Ok(project.combine(direct))
    }

    pub async fn machine_has_any_write(&self, machine: &MachineAccount) -> Result<bool, AppError> {
        if machine.compatibility_account {
            return Ok(true);
        }
        Ok(project_machine_grant::Entity::find()
            .filter(project_machine_grant::Column::MachineAccountId.eq(machine.id.to_string()))
            .filter(project_machine_grant::Column::CanWrite.eq(true))
            .one(self.db.connection())
            .await?
            .is_some())
    }

    pub async fn grant_machine_project(
        &self,
        machine_id: Uuid,
        project_id: Uuid,
        permission: Permission,
    ) -> Result<(), AppError> {
        validate_permission(permission)?;
        let timestamp = now();
        project_machine_grant::ActiveModel {
            project_id: Set(project_id.to_string()),
            machine_account_id: Set(machine_id.to_string()),
            can_read: Set(permission.read),
            can_write: Set(permission.write),
            created_at: Set(timestamp),
            updated_at: Set(timestamp),
        }
        .insert(self.db.connection())
        .await?;
        Ok(())
    }

    pub async fn project_view(&self, project_id: Uuid) -> Result<AccessPolicyView, AppError> {
        if project::Entity::find_by_id(project_id.to_string())
            .one(self.db.connection())
            .await?
            .is_none()
        {
            return Err(AppError::NotFound);
        }
        let users = project_user_grant::Entity::find()
            .filter(project_user_grant::Column::ProjectId.eq(project_id.to_string()))
            .all(self.db.connection())
            .await?;
        let groups = project_group_grant::Entity::find()
            .filter(project_group_grant::Column::ProjectId.eq(project_id.to_string()))
            .all(self.db.connection())
            .await?;
        let machines = project_machine_grant::Entity::find()
            .filter(project_machine_grant::Column::ProjectId.eq(project_id.to_string()))
            .all(self.db.connection())
            .await?;
        Ok(AccessPolicyView {
            users: map_user_grants(self.db.connection(), users, |grant| {
                (grant.user_id, grant.can_read, grant.can_write)
            })
            .await?,
            groups: map_group_grants(self.db.connection(), groups, |grant| {
                (grant.group_id, grant.can_read, grant.can_write)
            })
            .await?,
            machines: map_machine_grants(self.db.connection(), machines, |grant| {
                (grant.machine_account_id, grant.can_read, grant.can_write)
            })
            .await?,
        })
    }

    pub async fn secret_view(&self, secret_id: Uuid) -> Result<AccessPolicyView, AppError> {
        if secret::Entity::find_by_id(secret_id.to_string())
            .one(self.db.connection())
            .await?
            .is_none()
        {
            return Err(AppError::NotFound);
        }
        let users = secret_user_grant::Entity::find()
            .filter(secret_user_grant::Column::SecretId.eq(secret_id.to_string()))
            .all(self.db.connection())
            .await?;
        let groups = secret_group_grant::Entity::find()
            .filter(secret_group_grant::Column::SecretId.eq(secret_id.to_string()))
            .all(self.db.connection())
            .await?;
        let machines = secret_machine_grant::Entity::find()
            .filter(secret_machine_grant::Column::SecretId.eq(secret_id.to_string()))
            .all(self.db.connection())
            .await?;
        Ok(AccessPolicyView {
            users: map_user_grants(self.db.connection(), users, |grant| {
                (grant.user_id, grant.can_read, grant.can_write)
            })
            .await?,
            groups: map_group_grants(self.db.connection(), groups, |grant| {
                (grant.group_id, grant.can_read, grant.can_write)
            })
            .await?,
            machines: map_machine_grants(self.db.connection(), machines, |grant| {
                (grant.machine_account_id, grant.can_read, grant.can_write)
            })
            .await?,
        })
    }

    pub async fn machine_people_view(
        &self,
        machine_id: Uuid,
    ) -> Result<AccessPolicyView, AppError> {
        if machine_account::Entity::find_by_id(machine_id.to_string())
            .one(self.db.connection())
            .await?
            .is_none()
        {
            return Err(AppError::NotFound);
        }
        let users = machine_user_grant::Entity::find()
            .filter(machine_user_grant::Column::MachineAccountId.eq(machine_id.to_string()))
            .all(self.db.connection())
            .await?;
        let groups = machine_group_grant::Entity::find()
            .filter(machine_group_grant::Column::MachineAccountId.eq(machine_id.to_string()))
            .all(self.db.connection())
            .await?;
        Ok(AccessPolicyView {
            users: map_user_grants(self.db.connection(), users, |grant| {
                (grant.user_id, grant.can_read, grant.can_write)
            })
            .await?,
            groups: map_group_grants(self.db.connection(), groups, |grant| {
                (grant.group_id, grant.can_read, grant.can_write)
            })
            .await?,
            machines: Vec::new(),
        })
    }

    pub async fn machine_access_view(
        &self,
        machine_id: Uuid,
    ) -> Result<MachineAccessView, AppError> {
        let people = self.machine_people_view(machine_id).await?;
        Ok(MachineAccessView {
            users: people.users,
            groups: people.groups,
            projects: self.machine_granted_projects(machine_id).await?,
        })
    }

    pub async fn replace_project(
        &self,
        project_id: Uuid,
        input: &AccessPolicyInput,
    ) -> Result<AccessPolicyView, AppError> {
        validate_policy(input)?;
        let transaction = self.db.connection().begin().await?;
        require_exists::<project::Entity>(&transaction, project_id).await?;
        validate_grantees(&transaction, input).await?;
        project_user_grant::Entity::delete_many()
            .filter(project_user_grant::Column::ProjectId.eq(project_id.to_string()))
            .exec(&transaction)
            .await?;
        project_group_grant::Entity::delete_many()
            .filter(project_group_grant::Column::ProjectId.eq(project_id.to_string()))
            .exec(&transaction)
            .await?;
        project_machine_grant::Entity::delete_many()
            .filter(project_machine_grant::Column::ProjectId.eq(project_id.to_string()))
            .exec(&transaction)
            .await?;
        insert_project_grants(&transaction, project_id, input).await?;
        next_sdk_revision(&transaction).await?;
        transaction.commit().await?;
        self.project_view(project_id).await
    }

    pub async fn replace_secret(
        &self,
        secret_id: Uuid,
        input: &AccessPolicyInput,
    ) -> Result<AccessPolicyView, AppError> {
        validate_policy(input)?;
        let transaction = self.db.connection().begin().await?;
        require_exists::<secret::Entity>(&transaction, secret_id).await?;
        validate_grantees(&transaction, input).await?;
        secret_user_grant::Entity::delete_many()
            .filter(secret_user_grant::Column::SecretId.eq(secret_id.to_string()))
            .exec(&transaction)
            .await?;
        secret_group_grant::Entity::delete_many()
            .filter(secret_group_grant::Column::SecretId.eq(secret_id.to_string()))
            .exec(&transaction)
            .await?;
        secret_machine_grant::Entity::delete_many()
            .filter(secret_machine_grant::Column::SecretId.eq(secret_id.to_string()))
            .exec(&transaction)
            .await?;
        insert_secret_grants(&transaction, secret_id, input).await?;
        next_sdk_revision(&transaction).await?;
        transaction.commit().await?;
        self.secret_view(secret_id).await
    }

    pub async fn replace_machine_people(
        &self,
        machine_id: Uuid,
        users: &[GrantInput],
        groups: &[GrantInput],
    ) -> Result<AccessPolicyView, AppError> {
        validate_grant_list(users)?;
        validate_grant_list(groups)?;
        let input = AccessPolicyInput {
            users: users.to_vec(),
            groups: groups.to_vec(),
            machines: Vec::new(),
        };
        let transaction = self.db.connection().begin().await?;
        require_exists::<machine_account::Entity>(&transaction, machine_id).await?;
        validate_grantees(&transaction, &input).await?;
        machine_user_grant::Entity::delete_many()
            .filter(machine_user_grant::Column::MachineAccountId.eq(machine_id.to_string()))
            .exec(&transaction)
            .await?;
        machine_group_grant::Entity::delete_many()
            .filter(machine_group_grant::Column::MachineAccountId.eq(machine_id.to_string()))
            .exec(&transaction)
            .await?;
        let timestamp = now();
        for grant in unique_grants(users)?.into_iter().filter(|grant| grant.read) {
            machine_user_grant::ActiveModel {
                machine_account_id: Set(machine_id.to_string()),
                user_id: Set(grant.grantee_id.to_string()),
                can_read: Set(grant.read),
                can_write: Set(grant.write),
                created_at: Set(timestamp),
                updated_at: Set(timestamp),
            }
            .insert(&transaction)
            .await?;
        }
        for grant in unique_grants(groups)?
            .into_iter()
            .filter(|grant| grant.read)
        {
            machine_group_grant::ActiveModel {
                machine_account_id: Set(machine_id.to_string()),
                group_id: Set(grant.grantee_id.to_string()),
                can_read: Set(grant.read),
                can_write: Set(grant.write),
                created_at: Set(timestamp),
                updated_at: Set(timestamp),
            }
            .insert(&transaction)
            .await?;
        }
        transaction.commit().await?;
        self.machine_people_view(machine_id).await
    }

    pub async fn machine_granted_projects(
        &self,
        machine_id: Uuid,
    ) -> Result<Vec<NamedGrant>, AppError> {
        let grants = project_machine_grant::Entity::find()
            .filter(project_machine_grant::Column::MachineAccountId.eq(machine_id.to_string()))
            .all(self.db.connection())
            .await?;
        let mut result = Vec::with_capacity(grants.len());
        for grant in grants {
            let project = project::Entity::find_by_id(&grant.project_id)
                .one(self.db.connection())
                .await?
                .ok_or_else(|| AppError::internal(anyhow::anyhow!("grant project missing")))?;
            result.push(NamedGrant {
                grantee_id: Uuid::parse_str(&project.id).map_err(AppError::internal)?,
                name: project
                    .name_plain
                    .or(project.name_cipher)
                    .unwrap_or_else(|| "Project".into()),
                read: grant.can_read,
                write: grant.can_write,
            });
        }
        Ok(result)
    }

    pub async fn replace_machine_projects(
        &self,
        machine_id: Uuid,
        grants: &[GrantInput],
    ) -> Result<Vec<NamedGrant>, AppError> {
        validate_grant_list(grants)?;
        let transaction = self.db.connection().begin().await?;
        require_exists::<machine_account::Entity>(&transaction, machine_id).await?;
        let unique = unique_grants(grants)?;
        let projects = project::Entity::find()
            .filter(
                project::Column::Id.is_in(unique.iter().map(|grant| grant.grantee_id.to_string())),
            )
            .all(&transaction)
            .await?;
        if projects.len() != unique.len() {
            return Err(AppError::Validation(
                "policy contains an unknown project".into(),
            ));
        }
        project_machine_grant::Entity::delete_many()
            .filter(project_machine_grant::Column::MachineAccountId.eq(machine_id.to_string()))
            .exec(&transaction)
            .await?;
        let timestamp = now();
        for grant in unique.into_iter().filter(|grant| grant.read) {
            project_machine_grant::ActiveModel {
                project_id: Set(grant.grantee_id.to_string()),
                machine_account_id: Set(machine_id.to_string()),
                can_read: Set(grant.read),
                can_write: Set(grant.write),
                created_at: Set(timestamp),
                updated_at: Set(timestamp),
            }
            .insert(&transaction)
            .await?;
        }
        next_sdk_revision(&transaction).await?;
        transaction.commit().await?;
        self.machine_granted_projects(machine_id).await
    }

    pub async fn replace_machine_access(
        &self,
        machine_id: Uuid,
        input: &MachineAccessInput,
    ) -> Result<MachineAccessView, AppError> {
        validate_grant_list(&input.users)?;
        validate_grant_list(&input.groups)?;
        validate_grant_list(&input.projects)?;
        let transaction = self.db.connection().begin().await?;
        require_exists::<machine_account::Entity>(&transaction, machine_id).await?;
        validate_grantees(
            &transaction,
            &AccessPolicyInput {
                users: input.users.clone(),
                groups: input.groups.clone(),
                machines: Vec::new(),
            },
        )
        .await?;
        let projects = unique_grants(&input.projects)?;
        let found_projects = project::Entity::find()
            .filter(
                project::Column::Id
                    .is_in(projects.iter().map(|grant| grant.grantee_id.to_string())),
            )
            .all(&transaction)
            .await?;
        if found_projects.len() != projects.len() {
            return Err(AppError::Validation(
                "policy contains an unknown project".into(),
            ));
        }

        machine_user_grant::Entity::delete_many()
            .filter(machine_user_grant::Column::MachineAccountId.eq(machine_id.to_string()))
            .exec(&transaction)
            .await?;
        machine_group_grant::Entity::delete_many()
            .filter(machine_group_grant::Column::MachineAccountId.eq(machine_id.to_string()))
            .exec(&transaction)
            .await?;
        project_machine_grant::Entity::delete_many()
            .filter(project_machine_grant::Column::MachineAccountId.eq(machine_id.to_string()))
            .exec(&transaction)
            .await?;

        let timestamp = now();
        for grant in unique_grants(&input.users)?
            .into_iter()
            .filter(|grant| grant.read)
        {
            machine_user_grant::ActiveModel {
                machine_account_id: Set(machine_id.to_string()),
                user_id: Set(grant.grantee_id.to_string()),
                can_read: Set(true),
                can_write: Set(grant.write),
                created_at: Set(timestamp),
                updated_at: Set(timestamp),
            }
            .insert(&transaction)
            .await?;
        }
        for grant in unique_grants(&input.groups)?
            .into_iter()
            .filter(|grant| grant.read)
        {
            machine_group_grant::ActiveModel {
                machine_account_id: Set(machine_id.to_string()),
                group_id: Set(grant.grantee_id.to_string()),
                can_read: Set(true),
                can_write: Set(grant.write),
                created_at: Set(timestamp),
                updated_at: Set(timestamp),
            }
            .insert(&transaction)
            .await?;
        }
        for grant in projects.into_iter().filter(|grant| grant.read) {
            project_machine_grant::ActiveModel {
                project_id: Set(grant.grantee_id.to_string()),
                machine_account_id: Set(machine_id.to_string()),
                can_read: Set(true),
                can_write: Set(grant.write),
                created_at: Set(timestamp),
                updated_at: Set(timestamp),
            }
            .insert(&transaction)
            .await?;
        }
        next_sdk_revision(&transaction).await?;
        transaction.commit().await?;
        self.machine_access_view(machine_id).await
    }
}

async fn permission_query(
    connection: &impl ConnectionTrait,
    sql: &str,
    values: impl IntoIterator<Item = sea_orm::Value>,
) -> Result<Permission, AppError> {
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            sql,
            values,
        ))
        .await?
        .ok_or_else(|| AppError::internal(anyhow::anyhow!("permission query returned no row")))?;
    Ok(Permission {
        read: row.try_get::<i64>("", "can_read")? != 0,
        write: row.try_get::<i64>("", "can_write")? != 0,
    })
}

fn validate_permission(permission: Permission) -> Result<(), AppError> {
    if permission.write && !permission.read {
        Err(AppError::Validation(
            "write permission requires read".into(),
        ))
    } else {
        Ok(())
    }
}

fn validate_grant_list(grants: &[GrantInput]) -> Result<(), AppError> {
    for grant in grants {
        validate_permission(Permission {
            read: grant.read,
            write: grant.write,
        })?;
    }
    unique_grants(grants).map(|_| ())
}

fn validate_policy(input: &AccessPolicyInput) -> Result<(), AppError> {
    validate_grant_list(&input.users)?;
    validate_grant_list(&input.groups)?;
    validate_grant_list(&input.machines)
}

fn unique_grants(grants: &[GrantInput]) -> Result<Vec<GrantInput>, AppError> {
    let mut result = grants.to_vec();
    result.sort_by_key(|grant| grant.grantee_id);
    if result
        .windows(2)
        .any(|window| window[0].grantee_id == window[1].grantee_id)
    {
        return Err(AppError::Validation(
            "policy contains a duplicate grantee".into(),
        ));
    }
    Ok(result)
}

async fn require_exists<E>(
    transaction: &sea_orm::DatabaseTransaction,
    id: Uuid,
) -> Result<(), AppError>
where
    E: EntityTrait,
    E::PrimaryKey: sea_orm::PrimaryKeyTrait<ValueType = String>,
{
    E::find_by_id(id.to_string())
        .one(transaction)
        .await?
        .map(|_| ())
        .ok_or(AppError::NotFound)
}

async fn validate_grantees(
    transaction: &sea_orm::DatabaseTransaction,
    input: &AccessPolicyInput,
) -> Result<(), AppError> {
    let users = unique_grants(&input.users)?;
    let groups = unique_grants(&input.groups)?;
    let machines = unique_grants(&input.machines)?;
    let found_users = user::Entity::find()
        .filter(user::Column::Id.is_in(users.iter().map(|grant| grant.grantee_id.to_string())))
        .all(transaction)
        .await?;
    let found_groups = group::Entity::find()
        .filter(group::Column::Id.is_in(groups.iter().map(|grant| grant.grantee_id.to_string())))
        .all(transaction)
        .await?;
    let found_machines = machine_account::Entity::find()
        .filter(
            machine_account::Column::Id
                .is_in(machines.iter().map(|grant| grant.grantee_id.to_string())),
        )
        .all(transaction)
        .await?;
    if found_users.len() != users.len()
        || found_groups.len() != groups.len()
        || found_machines.len() != machines.len()
    {
        return Err(AppError::Validation(
            "policy contains an unknown grantee".into(),
        ));
    }
    Ok(())
}

async fn insert_project_grants(
    transaction: &sea_orm::DatabaseTransaction,
    project_id: Uuid,
    input: &AccessPolicyInput,
) -> Result<(), AppError> {
    let timestamp = now();
    for grant in unique_grants(&input.users)?
        .into_iter()
        .filter(|grant| grant.read)
    {
        project_user_grant::ActiveModel {
            project_id: Set(project_id.to_string()),
            user_id: Set(grant.grantee_id.to_string()),
            can_read: Set(grant.read),
            can_write: Set(grant.write),
            created_at: Set(timestamp),
            updated_at: Set(timestamp),
        }
        .insert(transaction)
        .await?;
    }
    for grant in unique_grants(&input.groups)?
        .into_iter()
        .filter(|grant| grant.read)
    {
        project_group_grant::ActiveModel {
            project_id: Set(project_id.to_string()),
            group_id: Set(grant.grantee_id.to_string()),
            can_read: Set(grant.read),
            can_write: Set(grant.write),
            created_at: Set(timestamp),
            updated_at: Set(timestamp),
        }
        .insert(transaction)
        .await?;
    }
    for grant in unique_grants(&input.machines)?
        .into_iter()
        .filter(|grant| grant.read)
    {
        project_machine_grant::ActiveModel {
            project_id: Set(project_id.to_string()),
            machine_account_id: Set(grant.grantee_id.to_string()),
            can_read: Set(grant.read),
            can_write: Set(grant.write),
            created_at: Set(timestamp),
            updated_at: Set(timestamp),
        }
        .insert(transaction)
        .await?;
    }
    Ok(())
}

async fn insert_secret_grants(
    transaction: &sea_orm::DatabaseTransaction,
    secret_id: Uuid,
    input: &AccessPolicyInput,
) -> Result<(), AppError> {
    let timestamp = now();
    for grant in unique_grants(&input.users)?
        .into_iter()
        .filter(|grant| grant.read)
    {
        secret_user_grant::ActiveModel {
            secret_id: Set(secret_id.to_string()),
            user_id: Set(grant.grantee_id.to_string()),
            can_read: Set(grant.read),
            can_write: Set(grant.write),
            created_at: Set(timestamp),
            updated_at: Set(timestamp),
        }
        .insert(transaction)
        .await?;
    }
    for grant in unique_grants(&input.groups)?
        .into_iter()
        .filter(|grant| grant.read)
    {
        secret_group_grant::ActiveModel {
            secret_id: Set(secret_id.to_string()),
            group_id: Set(grant.grantee_id.to_string()),
            can_read: Set(grant.read),
            can_write: Set(grant.write),
            created_at: Set(timestamp),
            updated_at: Set(timestamp),
        }
        .insert(transaction)
        .await?;
    }
    for grant in unique_grants(&input.machines)?
        .into_iter()
        .filter(|grant| grant.read)
    {
        secret_machine_grant::ActiveModel {
            secret_id: Set(secret_id.to_string()),
            machine_account_id: Set(grant.grantee_id.to_string()),
            can_read: Set(grant.read),
            can_write: Set(grant.write),
            created_at: Set(timestamp),
            updated_at: Set(timestamp),
        }
        .insert(transaction)
        .await?;
    }
    Ok(())
}

async fn map_user_grants<T>(
    connection: &impl ConnectionTrait,
    grants: Vec<T>,
    fields: impl Fn(T) -> (String, bool, bool),
) -> Result<Vec<NamedGrant>, AppError> {
    let mut result = Vec::with_capacity(grants.len());
    for grant in grants {
        let (id, read, write) = fields(grant);
        let user = user::Entity::find_by_id(&id)
            .one(connection)
            .await?
            .ok_or_else(|| AppError::internal(anyhow::anyhow!("grant user missing")))?;
        result.push(NamedGrant {
            grantee_id: Uuid::parse_str(&id).map_err(AppError::internal)?,
            name: user.display_name,
            read,
            write,
        });
    }
    Ok(result)
}

async fn map_group_grants<T>(
    connection: &impl ConnectionTrait,
    grants: Vec<T>,
    fields: impl Fn(T) -> (String, bool, bool),
) -> Result<Vec<NamedGrant>, AppError> {
    let mut result = Vec::with_capacity(grants.len());
    for grant in grants {
        let (id, read, write) = fields(grant);
        let group = group::Entity::find_by_id(&id)
            .one(connection)
            .await?
            .ok_or_else(|| AppError::internal(anyhow::anyhow!("grant group missing")))?;
        result.push(NamedGrant {
            grantee_id: Uuid::parse_str(&id).map_err(AppError::internal)?,
            name: group.name,
            read,
            write,
        });
    }
    Ok(result)
}

async fn map_machine_grants<T>(
    connection: &impl ConnectionTrait,
    grants: Vec<T>,
    fields: impl Fn(T) -> (String, bool, bool),
) -> Result<Vec<NamedGrant>, AppError> {
    let mut result = Vec::with_capacity(grants.len());
    for grant in grants {
        let (id, read, write) = fields(grant);
        let machine = machine_account::Entity::find_by_id(&id)
            .one(connection)
            .await?
            .ok_or_else(|| AppError::internal(anyhow::anyhow!("grant machine missing")))?;
        result.push(NamedGrant {
            grantee_id: Uuid::parse_str(&id).map_err(AppError::internal)?,
            name: machine.name,
            read,
            write,
        });
    }
    Ok(result)
}
