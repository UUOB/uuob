//! Keys and encryption at rest.
//!
//! A 32-byte master key and the device's own key are files in
//! `<data dir>/keys`, readable only by the user. Every container has its own
//! random key, stored wrapped by the master key, so deleting a container's
//! key makes any leftover bytes unreadable. The keys sit next to the data:
//! encryption at rest protects a copy of the database that is taken without
//! the keys folder, not a copy of the whole data folder.

use std::path::Path;

use anyhow::Context;
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use data_encoding::{HEXLOWER, HEXLOWER_PERMISSIVE};
use rand::RngCore;

use crate::error::{Error, Result};

/// Load the key `<dir>/<name>.key`, creating a random one when missing.
pub(crate) fn load_or_create_key(dir: &Path, name: &str) -> Result<[u8; 32]> {
    std::fs::create_dir_all(dir).context("create key dir")?;
    let path = dir.join(format!("{name}.key"));
    if let Ok(text) = std::fs::read_to_string(&path) {
        let bytes = HEXLOWER_PERMISSIVE.decode(text.trim().as_bytes()).context("decode key")?;
        return bytes.try_into().map_err(|_| Error::Invalid("bad key length".into()));
    }
    let key = random32();
    write_secret(&path, HEXLOWER.encode(&key).as_bytes()).context("write key")?;
    Ok(key)
}

/// Write a file only its owner can read, from the moment it exists.
pub(crate) fn write_secret(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)?.write_all(bytes)
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
    HEXLOWER.encode(&sha2::Sha256::digest(data))
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
