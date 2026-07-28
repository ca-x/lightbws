use anyhow::{Context, Result};
use bitwarden::secrets_manager::{
    AccessTokenLoginRequest, ClientSettings, SecretsManagerClient,
    projects::{ProjectCreateRequest, ProjectsDeleteRequest, ProjectsListRequest},
    secrets::{SecretCreateRequest, SecretGetRequest, SecretIdentifiersRequest, SecretsDeleteRequest},
};

#[tokio::main]
async fn main() -> Result<()> {
    let server_url = std::env::var("LIGHTBWS_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".into());
    let access_token = std::env::var("BWS_ACCESS_TOKEN").context("BWS_ACCESS_TOKEN is required")?;
    let client = SecretsManagerClient::new(Some(ClientSettings {
        identity_url: format!("{}/identity", server_url.trim_end_matches('/')),
        api_url: format!("{}/api", server_url.trim_end_matches('/')),
        ..Default::default()
    }));
    client.auth().login_access_token(&AccessTokenLoginRequest { access_token, state_file: None }).await?;
    let organization_id = client.get_access_token_organization().context("token has no organization")?;

    let project = client.projects().create(&ProjectCreateRequest {
        organization_id: organization_id.into(),
        name: "LightBWS SDK acceptance".into(),
    }).await?;
    let secret = client.secrets().create(&SecretCreateRequest {
        organization_id: organization_id.into(),
        key: "LIGHTBWS_DEMO".into(),
        value: "sdk-round-trip-ok".into(),
        note: "Created by demo/sdk-demo".into(),
        project_ids: Some(vec![project.id]),
    }).await?;
    let fetched = client.secrets().get(&SecretGetRequest { id: secret.id }).await?;
    anyhow::ensure!(fetched.value == "sdk-round-trip-ok", "secret round-trip mismatch");

    let projects = client.projects().list(&ProjectsListRequest { organization_id: organization_id.into() }).await?;
    let secrets = client.secrets().list(&SecretIdentifiersRequest { organization_id: organization_id.into() }).await?;
    println!("SDK round trip passed: {} projects, {} secrets", projects.data.len(), secrets.data.len());

    client.secrets().delete(SecretsDeleteRequest { ids: vec![secret.id] }).await?;
    client.projects().delete(ProjectsDeleteRequest { ids: vec![project.id] }).await?;
    Ok(())
}
