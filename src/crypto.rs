use std::{fs::OpenOptions, io::Write, path::Path, sync::Arc};

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, Payload},
};
use anyhow::{Context, Result, bail};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::TryRng;
use secrecy::ExposeSecret;

use crate::config::Config;

const NONCE_LEN: usize = 12;

#[derive(Clone)]
pub struct MasterKey(Arc<[u8; 32]>);

impl MasterKey {
    pub fn load_or_create(config: &Config) -> Result<Self> {
        if let Some(value) = &config.master_key {
            return Self::parse(value.expose_secret())
                .context("LIGHTBWS_MASTER_KEY must be base64url or 64 hexadecimal characters");
        }
        let path = config.master_key_path();
        if path.exists() {
            let value = std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            return Self::parse(value.trim())
                .with_context(|| format!("invalid master key in {}", path.display()));
        }
        let key = Self::random()?;
        write_new_key(&path, &URL_SAFE_NO_PAD.encode(key.bytes()))?;
        Ok(key)
    }

    pub fn random() -> Result<Self> {
        let mut bytes = [0_u8; 32];
        rand::rng()
            .try_fill_bytes(&mut bytes)
            .context("failed to obtain random bytes")?;
        Ok(Self(Arc::new(bytes)))
    }

    pub fn parse(value: &str) -> Result<Self> {
        let decoded = URL_SAFE_NO_PAD
            .decode(value)
            .or_else(|_| hex::decode(value))
            .context("invalid key encoding")?;
        let bytes: [u8; 32] = decoded
            .try_into()
            .map_err(|_| anyhow::anyhow!("master key must contain exactly 32 bytes"))?;
        Ok(Self(Arc::new(bytes)))
    }

    pub fn encrypt(&self, aad: &[u8], plaintext: &[u8]) -> Result<String> {
        let mut nonce = [0_u8; NONCE_LEN];
        rand::rng()
            .try_fill_bytes(&mut nonce)
            .context("failed to obtain random bytes")?;
        let cipher = Aes256Gcm::new_from_slice(self.bytes()).expect("AES-256 key length");
        let ciphertext = cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: plaintext,
                    aad,
                },
            )
            .map_err(|_| anyhow::anyhow!("encryption failed"))?;
        let mut envelope = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        envelope.extend_from_slice(&nonce);
        envelope.extend_from_slice(&ciphertext);
        Ok(URL_SAFE_NO_PAD.encode(envelope))
    }

    pub fn decrypt(&self, aad: &[u8], envelope: &str) -> Result<Vec<u8>> {
        let envelope = URL_SAFE_NO_PAD
            .decode(envelope)
            .context("invalid encrypted value")?;
        if envelope.len() <= NONCE_LEN {
            bail!("invalid encrypted value");
        }
        let (nonce, ciphertext) = envelope.split_at(NONCE_LEN);
        Aes256Gcm::new_from_slice(self.bytes())
            .expect("AES-256 key length")
            .decrypt(
                Nonce::from_slice(nonce),
                Payload {
                    msg: ciphertext,
                    aad,
                },
            )
            .map_err(|_| anyhow::anyhow!("decryption failed"))
    }

    pub(crate) fn bytes(&self) -> &[u8; 32] {
        self.0.as_ref()
    }
}

fn write_new_key(path: &Path, value: &str) -> Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .with_context(|| format!("failed to create {}", path.display()))?;
    file.write_all(value.as_bytes())
        .with_context(|| format!("failed to write {}", path.display()))?;
    file.write_all(b"\n")
        .with_context(|| format!("failed to write {}", path.display()))?;
    file.sync_all()
        .with_context(|| format!("failed to sync {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::MasterKey;

    #[test]
    fn binds_ciphertext_to_associated_data() {
        let key = MasterKey::random().unwrap();
        let ciphertext = key.encrypt(b"target-a", b"secret").unwrap();
        assert_eq!(key.decrypt(b"target-a", &ciphertext).unwrap(), b"secret");
        assert!(key.decrypt(b"target-b", &ciphertext).is_err());
    }
}
