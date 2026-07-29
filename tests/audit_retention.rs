use lightbws::{
    db::Database,
    domain::audit::{AuditActor, AuditRepository, UpdateAuditSettings},
};
use sea_orm::ConnectionTrait;

async fn database() -> (tempfile::TempDir, Database) {
    let data = tempfile::tempdir().expect("tempdir");
    let db = Database::connect(&data.path().join("audit-retention.sqlite3"))
        .await
        .expect("database");
    (data, db)
}

#[tokio::test]
async fn audit_logging_can_be_disabled_and_manually_cleared() {
    let (_data, db) = database().await;
    let repository = AuditRepository::new(db);
    repository
        .record(
            AuditActor::System,
            "audit.enabled",
            "audit",
            None,
            "allowed",
        )
        .await
        .expect("first event");
    repository
        .update_settings(UpdateAuditSettings {
            enabled: false,
            auto_cleanup_enabled: true,
            retention_days: 30,
        })
        .await
        .expect("disable audit logging");
    repository
        .record(
            AuditActor::System,
            "audit.disabled",
            "audit",
            None,
            "allowed",
        )
        .await
        .expect("disabled logging is a no-op");
    assert_eq!(repository.list().await.expect("events").len(), 1);
    assert_eq!(repository.clear().await.expect("clear events"), 1);
    assert!(repository.list().await.expect("cleared events").is_empty());
}

#[tokio::test]
async fn automatic_cleanup_deletes_only_expired_events() {
    let (_data, db) = database().await;
    let repository = AuditRepository::new(db.clone());
    db.connection()
        .execute_unprepared(
            r#"
            INSERT INTO audit_events (
                id, actor_kind, actor_id, action, resource_kind, resource_id, outcome, created_at
            ) VALUES (
                '00000000-0000-4000-8000-000000000001', 'system', NULL,
                'audit.expired', 'audit', NULL, 'allowed', 1
            )
            "#,
        )
        .await
        .expect("old event");
    repository
        .record(
            AuditActor::System,
            "audit.current",
            "audit",
            None,
            "allowed",
        )
        .await
        .expect("current event");

    assert_eq!(repository.cleanup_if_due().await.expect("cleanup"), 1);
    let events = repository.list().await.expect("remaining events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].action, "audit.current");
    assert_eq!(
        repository
            .cleanup_if_due()
            .await
            .expect("rate-limited cleanup"),
        0
    );
    assert!(
        repository
            .settings()
            .await
            .expect("settings")
            .last_cleanup_at
            .is_some()
    );
}
