use std::{net::SocketAddr, path::PathBuf};

use anyhow::{Context, Result, bail};
use secrecy::SecretString;

#[derive(Clone)]
pub struct BootstrapAdmin {
    pub username: String,
    pub password: SecretString,
}

#[derive(Clone)]
pub struct Config {
    pub bind: SocketAddr,
    pub data_dir: PathBuf,
    pub bootstrap_admin: Option<BootstrapAdmin>,
    pub cookie_secure: bool,
    pub upstream_compatibility_account: bool,
    pub allow_plaintext_backups: bool,
    pub master_key: Option<SecretString>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let bind = env_value("LIGHTBWS_BIND")
            .unwrap_or_else(|| "0.0.0.0:8080".into())
            .parse()
            .context("LIGHTBWS_BIND is invalid")?;
        let data_dir =
            PathBuf::from(env_value("LIGHTBWS_DATA_DIR").unwrap_or_else(|| "data".into()));
        let username = env_value("LIGHTBWS_ADMIN_USERNAME");
        let password = env_value("LIGHTBWS_ADMIN_PASSWORD");
        let bootstrap_admin = match (username, password) {
            (Some(username), Some(password)) => Some(BootstrapAdmin {
                username: validate_username(&username)?.to_owned(),
                password: SecretString::from(validate_password(&password)?.to_owned()),
            }),
            (None, None) => None,
            _ => bail!("LIGHTBWS_ADMIN_USERNAME and LIGHTBWS_ADMIN_PASSWORD must be set together"),
        };
        let cookie_secure = parse_bool("LIGHTBWS_COOKIE_SECURE", false)?;
        let upstream_compatibility_account =
            parse_bool("LIGHTBWS_ENABLE_UPSTREAM_COMPATIBILITY_ACCOUNT", false)?;
        let allow_plaintext_backups = parse_bool("LIGHTBWS_ALLOW_PLAINTEXT_BACKUPS", false)?;
        let master_key = env_value("LIGHTBWS_MASTER_KEY").map(SecretString::from);
        Ok(Self {
            bind,
            data_dir,
            bootstrap_admin,
            cookie_secure,
            upstream_compatibility_account,
            allow_plaintext_backups,
            master_key,
        })
    }

    pub fn database_path(&self) -> PathBuf {
        self.data_dir.join("lightbws.sqlite3")
    }

    pub fn master_key_path(&self) -> PathBuf {
        self.data_dir.join("master.key")
    }
}

pub fn validate_username(value: &str) -> Result<&str> {
    let value = value.trim();
    if !(1..=128).contains(&value.chars().count()) || value.chars().any(char::is_control) {
        bail!("administrator username is invalid");
    }
    Ok(value)
}

pub fn validate_password(value: &str) -> Result<&str> {
    if !(6..=4096).contains(&value.chars().count()) || value.chars().any(char::is_control) {
        bail!("administrator password must contain 6-4096 non-control characters");
    }
    Ok(value)
}

fn env_value(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn parse_bool(name: &str, default: bool) -> Result<bool> {
    match env_value(name).as_deref() {
        None => Ok(default),
        Some("1" | "true" | "yes" | "on") => Ok(true),
        Some("0" | "false" | "no" | "off") => Ok(false),
        Some(_) => bail!("{name} must be true or false"),
    }
}

impl std::fmt::Debug for Config {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Config")
            .field("bind", &self.bind)
            .field("data_dir", &self.data_dir)
            .field("bootstrap_admin", &self.bootstrap_admin.is_some())
            .field("cookie_secure", &self.cookie_secure)
            .field(
                "upstream_compatibility_account",
                &self.upstream_compatibility_account,
            )
            .field("allow_plaintext_backups", &self.allow_plaintext_backups)
            .field(
                "master_key",
                &self.master_key.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::{validate_password, validate_username};

    #[test]
    fn validates_bootstrap_credentials() {
        assert_eq!(validate_username(" admin ").unwrap(), "admin");
        assert!(validate_username("").is_err());
        assert!(validate_username("bad\nname").is_err());
        assert!(validate_password("123456").is_ok());
        assert!(validate_password("12345").is_err());
    }
}
