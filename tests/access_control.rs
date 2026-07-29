use lightbws::{
    db::Database,
    domain::{
        access::{AccessPolicyInput, AccessRepository, GrantInput, Permission},
        groups::GroupRepository,
        machines::MachineRepository,
        projects::ProjectRepository,
        secrets::SecretRepository,
        users::{Role, UserRepository},
    },
};

#[tokio::test]
async fn effective_permissions_combine_direct_group_project_and_secret_grants() {
    let data = tempfile::tempdir().expect("tempdir");
    let db = Database::connect(&data.path().join("access.sqlite3"))
        .await
        .expect("database");
    let users = UserRepository::new(db.clone());
    let admin = users
        .create("admin", "Administrator", Role::Admin, "test-password-123")
        .await
        .expect("admin");
    let developer = users
        .create("developer", "Developer", Role::User, "test-password-123")
        .await
        .expect("developer");
    let operator = users
        .create("operator", "Operator", Role::User, "test-password-123")
        .await
        .expect("operator");
    let group = GroupRepository::new(db.clone())
        .create("Platform")
        .await
        .expect("group");
    GroupRepository::new(db.clone())
        .replace_members(group.id, &[developer.id])
        .await
        .expect("membership");
    let project = ProjectRepository::new(db.clone())
        .create_plain("Production")
        .await
        .expect("project");
    let secret = SecretRepository::new(db.clone())
        .create_plain("DATABASE_URL", "sqlite://data", "runtime", project.id)
        .await
        .expect("secret");
    let issued = MachineRepository::new(db.clone())
        .issue("deploy", admin.id)
        .await
        .expect("machine");
    let access = AccessRepository::new(db.clone());

    assert_eq!(
        access
            .user_project(developer.id, developer.role, project.id)
            .await
            .expect("developer project"),
        Permission::default()
    );
    assert_eq!(
        access
            .machine_project(&issued.account, project.id)
            .await
            .expect("machine project"),
        Permission::default()
    );

    access
        .replace_project(
            project.id,
            &AccessPolicyInput {
                users: vec![],
                groups: vec![GrantInput {
                    grantee_id: group.id,
                    read: true,
                    write: false,
                }],
                machines: vec![GrantInput {
                    grantee_id: issued.account.id,
                    read: true,
                    write: false,
                }],
            },
        )
        .await
        .expect("project policy");
    assert_eq!(
        access
            .user_secret(
                developer.id,
                developer.role,
                &SecretRepository::new(db.clone())
                    .get(secret.id)
                    .await
                    .expect("stored secret")
            )
            .await
            .expect("group secret"),
        Permission {
            read: true,
            write: false
        }
    );

    access
        .replace_secret(
            secret.id,
            &AccessPolicyInput {
                users: vec![GrantInput {
                    grantee_id: operator.id,
                    read: true,
                    write: true,
                }],
                groups: vec![],
                machines: vec![GrantInput {
                    grantee_id: issued.account.id,
                    read: true,
                    write: true,
                }],
            },
        )
        .await
        .expect("secret policy");
    let model = SecretRepository::new(db.clone())
        .get(secret.id)
        .await
        .expect("secret");
    assert_eq!(
        access
            .user_secret(operator.id, operator.role, &model)
            .await
            .expect("direct user"),
        Permission::FULL
    );
    assert_eq!(
        access
            .machine_secret(&issued.account, &model)
            .await
            .expect("direct machine"),
        Permission::FULL
    );
    ProjectRepository::new(db.clone())
        .set_deleted(&[project.id], true)
        .await
        .expect("trash project");
    assert_eq!(
        access
            .machine_secret(&issued.account, &model)
            .await
            .expect("trashed project blocks direct machine access"),
        Permission::default()
    );
    ProjectRepository::new(db.clone())
        .set_deleted(&[project.id], false)
        .await
        .expect("restore project");
    assert_eq!(
        access
            .machine_secret(&issued.account, &model)
            .await
            .expect("restored project restores direct machine access"),
        Permission::FULL
    );

    GroupRepository::new(db.clone())
        .replace_members(group.id, &[])
        .await
        .expect("revoke membership");
    assert_eq!(
        access
            .user_project(developer.id, developer.role, project.id)
            .await
            .expect("revoked group"),
        Permission::default()
    );
}

#[tokio::test]
async fn invalid_policy_replacement_is_atomic() {
    let data = tempfile::tempdir().expect("tempdir");
    let db = Database::connect(&data.path().join("atomic.sqlite3"))
        .await
        .expect("database");
    let user = UserRepository::new(db.clone())
        .create("member", "Member", Role::User, "test-password-123")
        .await
        .expect("user");
    let project = ProjectRepository::new(db.clone())
        .create_plain("Atomic")
        .await
        .expect("project");
    let access = AccessRepository::new(db);
    access
        .replace_project(
            project.id,
            &AccessPolicyInput {
                users: vec![GrantInput {
                    grantee_id: user.id,
                    read: true,
                    write: false,
                }],
                groups: vec![],
                machines: vec![],
            },
        )
        .await
        .expect("initial policy");

    let rejected = access
        .replace_project(
            project.id,
            &AccessPolicyInput {
                users: vec![GrantInput {
                    grantee_id: user.id,
                    read: false,
                    write: true,
                }],
                groups: vec![],
                machines: vec![],
            },
        )
        .await;
    assert!(rejected.is_err());
    let view = access.project_view(project.id).await.expect("policy view");
    assert_eq!(view.users.len(), 1);
    assert!(view.users[0].read);
    assert!(!view.users[0].write);
}
