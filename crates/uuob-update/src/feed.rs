//! Reading an app's update feed and checking a download against its signature
//! (Android; computers leave both to Tauri's updater).

use base64::Engine;
use minisign_verify::{PublicKey, Signature};
use semver::Version;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
struct Feed {
    version: String,
    platforms: HashMap<String, File>,
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct File {
    pub url: String,
    pub signature: String,
}

/// The published version and its file for `target`, if it is newer than `current`.
pub fn newer(feed: &[u8], current: &Version, target: &str) -> Result<Option<(String, File)>, String> {
    let mut feed: Feed = serde_json::from_slice(feed).map_err(|e| format!("unreadable update feed: {e}"))?;
    let version = Version::parse(feed.version.trim_start_matches('v')).map_err(|e| format!("unreadable version: {e}"))?;
    if version <= *current {
        return Ok(None);
    }
    Ok(feed.platforms.remove(target).map(|file| (feed.version, file)))
}

/// Refuses `data` unless `signature` over it was made with the author's key.
/// Both are in the form Tauri's signer writes: base64 of minisign's text files.
pub fn verify(data: &[u8], signature: &str, pubkey: &str) -> Result<(), String> {
    let refused = |_| "This update isn’t signed by the app’s author, so it wasn’t installed.".to_string();
    let text = |s: &str| {
        let bytes = base64::engine::general_purpose::STANDARD.decode(s.trim()).map_err(|_| ())?;
        String::from_utf8(bytes).map_err(|_| ())
    };
    let key = PublicKey::decode(&text(pubkey).map_err(refused)?).map_err(|_| refused(()))?;
    let signature = Signature::decode(&text(signature).map_err(refused)?).map_err(|_| refused(()))?;
    key.verify(data, &signature, true).map_err(|_| refused(()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn b64(s: impl AsRef<[u8]>) -> String {
        base64::engine::general_purpose::STANDARD.encode(s)
    }

    /// A key pair and a signature over `data`, as Tauri's signer hands them out.
    fn signed(data: &[u8]) -> (String, String) {
        let pair = minisign::KeyPair::generate_unencrypted_keypair().unwrap();
        let signature = minisign::sign(Some(&pair.pk), &pair.sk, Cursor::new(data), None, None).unwrap();
        (b64(pair.pk.to_box().unwrap().to_string()), b64(signature.to_string()))
    }

    #[test]
    fn a_signed_file_passes() {
        let (key, signature) = signed(b"the app");
        assert_eq!(verify(b"the app", &signature, &key), Ok(()));
    }

    #[test]
    fn a_changed_file_is_refused() {
        let (key, signature) = signed(b"the app");
        assert!(verify(b"the app, tampered", &signature, &key).is_err());
    }

    #[test]
    fn another_authors_signature_is_refused() {
        let (key, _) = signed(b"the app");
        let (_, theirs) = signed(b"the app");
        assert!(verify(b"the app", &theirs, &key).is_err());
        assert!(verify(b"the app", "not a signature", &key).is_err());
    }

    #[test]
    fn only_a_newer_version_for_this_device_is_offered() {
        let feed = br#"{ "version": "0.1.2", "notes": "", "platforms": {
            "android-aarch64": { "url": "https://x/a.apk", "signature": "s" },
            "darwin-aarch64": { "url": "https://x/a.tar.gz", "signature": "t" } } }"#;
        let v = |s| Version::parse(s).unwrap();
        let file = File { url: "https://x/a.apk".into(), signature: "s".into() };
        assert_eq!(newer(feed, &v("0.1.1"), "android-aarch64"), Ok(Some(("0.1.2".into(), file))));
        assert_eq!(newer(feed, &v("0.1.2"), "android-aarch64"), Ok(None));
        assert_eq!(newer(feed, &v("0.2.0"), "android-aarch64"), Ok(None));
        assert_eq!(newer(feed, &v("0.1.1"), "android-x86_64"), Ok(None));
        assert!(newer(b"not a feed", &v("0.1.1"), "android-aarch64").is_err());
    }
}
