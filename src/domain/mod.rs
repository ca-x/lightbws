pub mod access;
pub mod audit;
pub mod backups;
pub mod groups;
pub mod machines;
pub mod projects;
pub mod secrets;
pub mod transfer;
pub mod users;

pub const ORGANIZATION_ID: &str = "f4e44a7f-1190-432a-9d4a-af96013127cb";

pub fn now() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

pub fn now_nanos() -> i64 {
    i64::try_from(time::OffsetDateTime::now_utc().unix_timestamp_nanos())
        .expect("current timestamp fits in i64")
}

pub async fn next_sdk_revision(
    connection: &impl sea_orm::ConnectionTrait,
) -> Result<i64, crate::error::AppError> {
    use sea_orm::{DatabaseBackend, Statement};

    let timestamp = now_nanos();
    let result = connection
        .query_one_raw(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"
            UPDATE sdk_sync_state
            SET revision_nanos = CASE
                WHEN revision_nanos >= ? THEN revision_nanos + 1
                ELSE ?
            END
            WHERE organization_id = ?
            RETURNING revision_nanos
            "#,
            [timestamp.into(), timestamp.into(), ORGANIZATION_ID.into()],
        ))
        .await?
        .ok_or_else(|| crate::error::AppError::internal(anyhow::anyhow!("SDK state is missing")))?;
    result
        .try_get("", "revision_nanos")
        .map_err(crate::error::AppError::from)
}
