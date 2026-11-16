//! Keys and encryption at rest.
//!
//! A 32-byte master key lives in a [`KeyStore`] (the OS keychain in the app,
//! a 0600 file for the headless node and tests). Every container has its own
//! random key, stored wrapped by the master key, so deleting a container's key
//! makes any leftover bytes unreadable.

use std::path::PathBuf;

use anyhow::Context;
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use rand::RngCore;

use crate::error::{Error, Result};

pub trait KeyStore: Send + Sync + 'static {
    /// Load the secret named `name`, creating a random one when missing.
    fn load_or_create(&self, name: &str) -> Result<[u8; 32]>;
}

/// Stores keys as files with owner-only permissions.
pub struct FileKeyStore {
    dir: PathBuf,
}

impl FileKeyStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }
}

impl KeyStore for FileKeyStore {
    fn load_or_create(&self, name: &str) -> Result<[u8; 32]> {
        std::fs::create_dir_all(&self.dir).context("create key dir")?;
        let path = self.dir.join(format!("{name}.key"));
        if let Ok(text) = std::fs::read_to_string(&path) {
            let bytes = hex::decode(text.trim()).context("decode key")?;
            return bytes.try_into().map_err(|_| Error::Invalid("bad key length".into()));
        }
        let key = random32();
        std::fs::write(&path, hex::encode(key)).context("write key")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(key)
    }
}

pub fn random32() -> [u8; 32] {
    let mut b = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut b);
    b
}

/// Random identifier, lowercase base32 without padding.
pub fn random_id(bytes: usize) -> String {
    let mut b = vec![0u8; bytes];
    rand::rngs::OsRng.fill_bytes(&mut b);
    data_encoding::BASE32_NOPAD.encode(&b).to_lowercase()
}

/// Encrypt with XChaCha20-Poly1305. Output: 24-byte nonce || ciphertext.
pub fn seal(key: &[u8; 32], plaintext: &[u8], aad: &[u8]) -> Vec<u8> {
    let cipher = XChaCha20Poly1305::new(key.into());
    let mut nonce = [0u8; 24];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let ct = cipher
        .encrypt(XNonce::from_slice(&nonce), Payload { msg: plaintext, aad })
        .expect("encryption cannot fail");
    let mut out = nonce.to_vec();
    out.extend_from_slice(&ct);
    out
}

pub fn open(key: &[u8; 32], data: &[u8], aad: &[u8]) -> Result<Vec<u8>> {
    if data.len() < 24 {
        return Err(Error::Invalid("ciphertext too short".into()));
    }
    let cipher = XChaCha20Poly1305::new(key.into());
    cipher
        .decrypt(XNonce::from_slice(&data[..24]), Payload { msg: &data[24..], aad })
        .map_err(|_| Error::Invalid("decryption failed (wrong key or corrupted data)".into()))
}

pub fn sha256_hex(data: &[u8]) -> String {
    use sha2::Digest;
    hex::encode(sha2::Sha256::digest(data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_open() {
        let k = random32();
        let ct = seal(&k, b"hello", b"tasks");
        assert_eq!(open(&k, &ct, b"tasks").unwrap(), b"hello");
        assert!(open(&k, &ct, b"calendar").is_err());
        assert!(open(&random32(), &ct, b"tasks").is_err());
    }
}
