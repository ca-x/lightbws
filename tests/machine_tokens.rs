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

fn access_token_id(access_token: &str) -> &str {
    access_token
        .split('.')
        .nth(1)
        .expect("machine access token id")
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
    let default_credential = repository
        .authenticate(
            access_token_id(&issued.access_token),
            client_secret(&issued.access_token),
        )
        .await
        .expect("default credential");
    let (default_session, _) = repository
        .create_session(&default_credential)
        .await
        .expect("default session");

    repository
        .revoke_access_token(issued.account.id, default_credential.token_id)
        .await
        .expect("revoke default token");
    assert!(
        repository
            .authenticate_session(&default_session)
            .await
            .is_err()
    );

    let second = repository
        .issue_access_token(issued.account.id, "CI", None)
        .await
        .expect("second token after default revocation");
    assert_eq!(
        access_token_id(&second.access_token),
        second.token.id.to_string()
    );
    let second_credential = repository
        .authenticate(
            access_token_id(&second.access_token),
            client_secret(&second.access_token),
        )
        .await
        .expect("new named credential authenticates");
    let (second_session, _) = repository
        .create_session(&second_credential)
        .await
        .expect("second session");

    repository
        .revoke_access_token(issued.account.id, second.token.id)
        .await
        .expect("revoke only second token");
    assert!(
        repository
            .authenticate_session(&second_session)
            .await
            .is_err()
    );
    assert!(
        repository
            .authenticate(
                access_token_id(&second.access_token),
                client_secret(&second.access_token),
            )
            .await
            .is_err()
    );
    let third = repository
        .issue_access_token(issued.account.id, "CI", None)
        .await
        .expect("revoked token name can be reused");
    let third_credential = repository
        .authenticate(
            access_token_id(&third.access_token),
            client_secret(&third.access_token),
        )
        .await
        .expect("replacement credential");
    let (third_session, _) = repository
        .create_session(&third_credential)
        .await
        .expect("replacement session");

    repository
        .set_revoked(issued.account.id, true)
        .await
        .expect("revoke account");
    assert!(
        repository
            .authenticate_session(&third_session)
            .await
            .is_err()
    );
    repository
        .set_revoked(issued.account.id, false)
        .await
        .expect("re-enable account");
    repository
        .authenticate_session(&third_session)
        .await
        .expect("existing token session resumes after account is re-enabled");
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
