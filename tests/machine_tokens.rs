use lightbws::{
    db::Database,
    domain::{
        machines::MachineRepository,
        users::{Role, UserRepository},
    },
};

fn client_secret(access_token: &str) -> &str {
    access_token
        .split('.')
        .nth(2)
        .and_then(|part| part.split(':').next())
        .expect("machine client secret")
}

#[tokio::test]
async fn named_machine_tokens_authenticate_and_revoke_independently() {
    let data = tempfile::tempdir().expect("tempdir");
    let db = Database::connect(&data.path().join("machine-tokens.sqlite3"))
        .await
        .expect("database");
    let admin = UserRepository::new(db.clone())
        .create("admin", "Administrator", Role::Admin, "123456")
        .await
        .expect("six-character administrator password");
    let repository = MachineRepository::new(db);
    let issued = repository
        .issue("Deployment", admin.id)
        .await
        .expect("machine account");
    let second = repository
        .issue_access_token(issued.account.id, "CI", None)
        .await
        .expect("second token");

    let default_credential = repository
        .authenticate(
            &issued.account.client_id.to_string(),
            client_secret(&issued.access_token),
        )
        .await
        .expect("default credential");
    let second_credential = repository
        .authenticate(
            &issued.account.client_id.to_string(),
            client_secret(&second.access_token),
        )
        .await
        .expect("second credential");
    let (default_session, _) = repository
        .create_session(&default_credential)
        .await
        .expect("default session");
    let (second_session, _) = repository
        .create_session(&second_credential)
        .await
        .expect("second session");

    repository
        .revoke_access_token(issued.account.id, second.token.id)
        .await
        .expect("revoke only second token");
    repository
        .authenticate_session(&default_session)
        .await
        .expect("default session remains valid");
    assert!(
        repository
            .authenticate_session(&second_session)
            .await
            .is_err()
    );
    assert!(
        repository
            .authenticate(
                &issued.account.client_id.to_string(),
                client_secret(&second.access_token),
            )
            .await
            .is_err()
    );
    repository
        .issue_access_token(issued.account.id, "CI", None)
        .await
        .expect("revoked token name can be reused");

    repository
        .set_revoked(issued.account.id, true)
        .await
        .expect("revoke account");
    assert!(
        repository
            .authenticate_session(&default_session)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn machine_token_expiry_must_be_in_the_future() {
    let data = tempfile::tempdir().expect("tempdir");
    let db = Database::connect(&data.path().join("machine-token-expiry.sqlite3"))
        .await
        .expect("database");
    let admin = UserRepository::new(db.clone())
        .create("admin", "Administrator", Role::Admin, "123456")
        .await
        .expect("admin");
    let repository = MachineRepository::new(db);
    let issued = repository
        .issue("Deployment", admin.id)
        .await
        .expect("machine account");

    assert!(
        repository
            .issue_access_token(issued.account.id, "Expired", Some(0))
            .await
            .is_err()
    );
}
