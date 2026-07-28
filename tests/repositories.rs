use lightbws::{
    db::Database,
    domain::{
        machines::MachineRepository,
        projects::{ProjectRepository, WebProject},
        secrets::{SecretRepository, WebSecret},
        users::{Role, UserRepository},
    },
};
use tempfile::TempDir;

async fn database(data: &TempDir) -> Database {
    Database::connect(&data.path().join("repository.sqlite3"))
        .await
        .expect("database")
}

#[tokio::test]
async fn resources_persist_across_reconnect_and_soft_delete() {
    let data = tempfile::tempdir().expect("tempdir");
    let db = database(&data).await;
    let user = UserRepository::new(db.clone())
        .create("admin", "Administrator", Role::Admin, "test-password-123")
        .await
        .expect("user");
    let project = ProjectRepository::new(db.clone())
        .create_plain("Production")
        .await
        .expect("project");
    let secret = SecretRepository::new(db.clone())
        .create_plain("DATABASE_URL", "sqlite://data", "runtime", Some(project.id))
        .await
        .expect("secret");
    let machine = MachineRepository::new(db.clone())
        .issue("deploy", user.id)
        .await
        .expect("machine");
    SecretRepository::new(db.clone())
        .set_deleted(&[secret.id], true)
        .await
        .expect("soft delete");
    drop(db);

    let reopened = database(&data).await;
    let projects = ProjectRepository::new(reopened.clone())
        .list(false)
        .await
        .expect("projects")
        .into_iter()
        .map(WebProject::try_from)
        .collect::<Result<Vec<_>, _>>()
        .expect("public projects");
    let active = SecretRepository::new(reopened.clone())
        .list(false, None)
        .await
        .expect("active secrets");
    let trash = SecretRepository::new(reopened.clone())
        .list(true, None)
        .await
        .expect("all secrets")
        .into_iter()
        .map(WebSecret::try_from)
        .collect::<Result<Vec<_>, _>>()
        .expect("public secrets");
    let machines = MachineRepository::new(reopened)
        .list()
        .await
        .expect("machines");
    assert_eq!(projects.len(), 1);
    assert!(active.is_empty());
    assert_eq!(trash.len(), 1);
    assert!(trash[0].deleted_at.is_some());
    assert_eq!(machines[0].id, machine.account.id);
}

#[tokio::test]
async fn unique_usernames_and_machine_names_are_enforced() {
    let data = tempfile::tempdir().expect("tempdir");
    let db = database(&data).await;
    let users = UserRepository::new(db.clone());
    let admin = users
        .create("Admin", "Administrator", Role::Admin, "test-password-123")
        .await
        .expect("admin");
    assert!(
        users
            .create("admin", "Duplicate", Role::User, "test-password-123")
            .await
            .is_err()
    );
    let machines = MachineRepository::new(db);
    machines.issue("release", admin.id).await.expect("machine");
    assert!(machines.issue("release", admin.id).await.is_err());
}

#[tokio::test]
async fn concurrent_updates_cannot_remove_every_active_administrator() {
    let data = tempfile::tempdir().expect("tempdir");
    let db = database(&data).await;
    let users = UserRepository::new(db);
    let first = users
        .create("admin-one", "Admin One", Role::Admin, "test-password-123")
        .await
        .expect("first admin");
    let second = users
        .create("admin-two", "Admin Two", Role::Admin, "test-password-123")
        .await
        .expect("second admin");

    let first_update = users.update(first.id, "Admin One", Role::User, false);
    let second_update = users.update(second.id, "Admin Two", Role::User, false);
    let (first_result, second_result) = tokio::join!(first_update, second_update);
    assert_eq!(
        usize::from(first_result.is_ok()) + usize::from(second_result.is_ok()),
        1
    );
    let active_admins = users
        .list()
        .await
        .expect("users")
        .into_iter()
        .filter(|user| user.role == Role::Admin && !user.disabled)
        .count();
    assert_eq!(active_admins, 1);
}

#[tokio::test]
async fn web_and_sdk_representations_remain_mutually_exclusive() {
    let data = tempfile::tempdir().expect("tempdir");
    let db = database(&data).await;
    let projects = ProjectRepository::new(db.clone());
    let project = projects
        .create_plain("Production")
        .await
        .expect("plain project");
    let cipher_project = projects
        .update_cipher(project.id, "2.project-ciphertext".into())
        .await
        .expect("cipher project");
    assert!(cipher_project.name_plain.is_none());
    let plain_project = projects
        .update_plain(project.id, "Recovered")
        .await
        .expect("plain project update");
    assert!(!plain_project.sdk_encrypted);
    assert!(
        projects
            .get(project.id)
            .await
            .expect("project")
            .name_cipher
            .is_none()
    );

    let secrets = SecretRepository::new(db);
    let secret = secrets
        .create_plain("TOKEN", "plain-value", "plain-note", Some(project.id))
        .await
        .expect("plain secret");
    let cipher_secret = secrets
        .update_cipher(
            secret.id,
            "2.key-ciphertext".into(),
            "2.value-ciphertext".into(),
            "2.note-ciphertext".into(),
            Some(project.id),
        )
        .await
        .expect("cipher secret");
    assert!(cipher_secret.key_plain.is_none());
    assert!(cipher_secret.value_plain.is_none());
    assert!(cipher_secret.note_plain.is_none());
    let plain_secret = secrets
        .update_plain(
            secret.id,
            "TOKEN_NEW",
            "new-plain-value",
            "new-plain-note",
            Some(project.id),
        )
        .await
        .expect("plain secret update");
    assert!(!plain_secret.sdk_encrypted);
    let stored = secrets.get(secret.id).await.expect("secret");
    assert!(stored.key_cipher.is_none());
    assert!(stored.value_cipher.is_none());
    assert!(stored.note_cipher.is_none());
}
