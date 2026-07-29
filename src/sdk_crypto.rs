use std::sync::LazyLock;

use aes::Aes256;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use cbc::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit, block_padding::Pkcs7};
use cbc::{Decryptor, Encryptor};
use hmac::{Hmac, Mac};
use rand::TryRng;
use serde::Deserialize;
use sha2::Sha256;
use subtle::ConstantTimeEq;

use crate::{domain::machines::SDK_ENCRYPTION_KEY, error::AppError};

type HmacSha256 = Hmac<Sha256>;

const ENCRYPTED_ORGANIZATION_PAYLOAD: &str = concat!(
    "2.E9fE8+M/VWMfhhim1KlCbQ==|eLsHR484S/tJbIkM6spnG/HP65tj9A6Tba7kAAvUp+rYuQmGLixiOCfMsqt5OvBctDfvvr/Aes",
    "Bu7cZimPLyOEhqEAjn52jF0eaI38XZfeOG2VJl0LOf60Wkfh3ryAMvfvLj3G4ZCNYU8sNgoC2+IQ==|lNApuCQ4Pyakfo/wwuuajWNaEX/2MW8/3rjXB/V7n+k="
);

static ORGANIZATION_KEY: LazyLock<[u8; 64]> = LazyLock::new(|| {
    load_organization_key().expect("embedded Bitwarden organization key payload must be valid")
});

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OrganizationPayload {
    encryption_key: String,
}

pub(crate) fn encrypt(value: &str) -> Result<String, AppError> {
    encrypt_with_key(value, &ORGANIZATION_KEY)
}

pub(crate) fn decrypt(value: &str) -> Result<String, AppError> {
    decrypt_with_key(value, &ORGANIZATION_KEY)
}

fn load_organization_key() -> Result<[u8; 64], AppError> {
    let raw_access_key = STANDARD
        .decode(SDK_ENCRYPTION_KEY)
        .map_err(AppError::internal)?;
    let raw_access_key: [u8; 16] = raw_access_key
        .try_into()
        .map_err(|_| AppError::internal(anyhow::anyhow!("invalid SDK access encryption key")))?;
    let access_key = derive_access_key(&raw_access_key)?;
    let payload = decrypt_with_key(ENCRYPTED_ORGANIZATION_PAYLOAD, &access_key)?;
    let payload: OrganizationPayload =
        serde_json::from_str(&payload).map_err(AppError::internal)?;
    let key = STANDARD
        .decode(payload.encryption_key)
        .map_err(AppError::internal)?;
    key.try_into()
        .map_err(|_| AppError::internal(anyhow::anyhow!("invalid SDK organization key")))
}

fn derive_access_key(secret: &[u8; 16]) -> Result<[u8; 64], AppError> {
    let mut extract = <HmacSha256 as Mac>::new_from_slice(b"bitwarden-accesstoken")
        .map_err(AppError::internal)?;
    extract.update(secret);
    let prk = extract.finalize().into_bytes();
    let info = b"sm-access-token";

    let mut first = <HmacSha256 as Mac>::new_from_slice(&prk).map_err(AppError::internal)?;
    first.update(info);
    first.update(&[1]);
    let first = first.finalize().into_bytes();

    let mut second = <HmacSha256 as Mac>::new_from_slice(&prk).map_err(AppError::internal)?;
    second.update(&first);
    second.update(info);
    second.update(&[2]);
    let second = second.finalize().into_bytes();

    let mut key = [0_u8; 64];
    key[..32].copy_from_slice(&first);
    key[32..].copy_from_slice(&second);
    Ok(key)
}

fn encrypt_with_key(value: &str, key: &[u8; 64]) -> Result<String, AppError> {
    let mut iv = [0_u8; 16];
    rand::rng()
        .try_fill_bytes(&mut iv)
        .map_err(AppError::internal)?;
    let ciphertext = Encryptor::<Aes256>::new_from_slices(&key[..32], &iv)
        .map_err(AppError::internal)?
        .encrypt_padded_vec_mut::<Pkcs7>(value.as_bytes());
    let mut hmac = <HmacSha256 as Mac>::new_from_slice(&key[32..]).map_err(AppError::internal)?;
    hmac.update(&iv);
    hmac.update(&ciphertext);
    let mac = hmac.finalize().into_bytes();
    Ok(format!(
        "2.{}|{}|{}",
        STANDARD.encode(iv),
        STANDARD.encode(ciphertext),
        STANDARD.encode(mac)
    ))
}

fn decrypt_with_key(value: &str, key: &[u8; 64]) -> Result<String, AppError> {
    let Some(value) = value.strip_prefix("2.") else {
        return Err(AppError::Validation("unsupported SDK ciphertext".into()));
    };
    let mut parts = value.split('|');
    let (Some(iv), Some(ciphertext), Some(mac), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(AppError::Validation("invalid SDK ciphertext".into()));
    };
    let iv = STANDARD.decode(iv).map_err(AppError::internal)?;
    let ciphertext = STANDARD.decode(ciphertext).map_err(AppError::internal)?;
    let mac = STANDARD.decode(mac).map_err(AppError::internal)?;
    if iv.len() != 16 || mac.len() != 32 {
        return Err(AppError::Validation("invalid SDK ciphertext".into()));
    }
    let mut expected =
        <HmacSha256 as Mac>::new_from_slice(&key[32..]).map_err(AppError::internal)?;
    expected.update(&iv);
    expected.update(&ciphertext);
    if expected
        .finalize()
        .into_bytes()
        .as_slice()
        .ct_eq(&mac)
        .unwrap_u8()
        != 1
    {
        return Err(AppError::Validation("invalid SDK ciphertext".into()));
    }
    let plaintext = Decryptor::<Aes256>::new_from_slices(&key[..32], &iv)
        .map_err(AppError::internal)?
        .decrypt_padded_vec_mut::<Pkcs7>(&ciphertext)
        .map_err(|_| AppError::Validation("invalid SDK ciphertext".into()))?;
    String::from_utf8(plaintext).map_err(AppError::internal)
}

#[cfg(test)]
mod tests {
    use super::{decrypt, encrypt};

    #[test]
    fn sdk_cipher_round_trip_uses_bitwarden_enc_string_format() {
        let encrypted = encrypt("Web SDK project").expect("encrypt");
        assert!(encrypted.starts_with("2."));
        assert_eq!(decrypt(&encrypted).expect("decrypt"), "Web SDK project");
    }

    #[test]
    fn decrypts_official_bitwarden_sdk_fixture() {
        let fixture = "2.pMS6/icTQABtulw52pq2lg==|XXbxKxDTh+mWiN1HjH2N1w==|Q6PkuT+KX/axrgN9ubD5Ajk2YNwxQkgs3WJM0S0wtG8=";
        assert_eq!(decrypt(fixture).expect("official fixture"), "TEST");
    }
}
