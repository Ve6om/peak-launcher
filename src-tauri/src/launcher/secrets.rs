//! Keeps sign-in tokens unreadable on disk. A random key lives in the
//! operating system's password store (Windows Credential Manager, macOS
//! Keychain, Linux Secret Service); tokens are saved encrypted with it, so a
//! copied data folder is useless on its own.
//!
//! The key, not the tokens, goes in the password store because Windows limits
//! each entry to a couple of kilobytes and Microsoft's tokens can be longer.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use std::sync::OnceLock;

const SERVICE: &str = "Peak Launcher";
const ENTRY: &str = "token-encryption-key";
/// Marks the format, so it can change later without guessing.
const PREFIX: &str = "v1:";

pub type SecretKey = [u8; 32];

static KEY: OnceLock<Option<SecretKey>> = OnceLock::new();

/// The encryption key from the OS password store, created on first use.
/// None when there's no password store (some minimal Linux setups).
pub fn key() -> Option<SecretKey> {
    *KEY.get_or_init(|| match load_or_create(ENTRY) {
        Ok(k) => Some(k),
        Err(e) => {
            eprintln!("No OS password store, sign-in tokens are stored unencrypted: {e}");
            None
        }
    })
}

fn load_or_create(name: &str) -> Result<SecretKey, String> {
    let entry = keyring::Entry::new(SERVICE, name).map_err(|e| e.to_string())?;
    match entry.get_password() {
        Ok(text) => {
            let bytes = B64.decode(text.trim()).map_err(|e| e.to_string())?;
            bytes.try_into().map_err(|_| "stored key has the wrong length".to_string())
        }
        Err(keyring::Error::NoEntry) => {
            let key: SecretKey = XChaCha20Poly1305::generate_key(&mut OsRng).into();
            entry.set_password(&B64.encode(key)).map_err(|e| e.to_string())?;
            // Read it back: some stores accept a write they can't return.
            let back = entry.get_password().map_err(|e| e.to_string())?;
            if B64.decode(back.trim()).ok().as_deref() != Some(&key[..]) {
                return Err("the password store didn't keep the key".into());
            }
            Ok(key)
        }
        Err(e) => Err(e.to_string()),
    }
}

/// Encrypt `plain`, tied to `context` (the account id) so an encrypted token
/// can't be moved onto another account.
pub fn seal(key: &SecretKey, context: &str, plain: &str) -> String {
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let data = cipher
        .encrypt(&nonce, Payload { msg: plain.as_bytes(), aad: context.as_bytes() })
        .expect("encryption with a valid key can't fail");
    let mut out = nonce.to_vec();
    out.extend_from_slice(&data);
    format!("{PREFIX}{}", B64.encode(out))
}

/// Decrypt what `seal` produced. None if the key or context doesn't match,
/// or the text was changed.
pub fn open(key: &SecretKey, context: &str, sealed: &str) -> Option<String> {
    let raw = B64.decode(sealed.strip_prefix(PREFIX)?).ok()?;
    if raw.len() < 24 {
        return None;
    }
    let (nonce, data) = raw.split_at(24);
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    let plain = cipher.decrypt(XNonce::from_slice(nonce), Payload { msg: data, aad: context.as_bytes() }).ok()?;
    String::from_utf8(plain).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let key = [7u8; 32];
        let sealed = seal(&key, "acc1", "M.C123_refresh-token");
        assert!(sealed.starts_with("v1:"));
        assert!(!sealed.contains("refresh"));
        assert_eq!(open(&key, "acc1", &sealed).as_deref(), Some("M.C123_refresh-token"));
    }

    #[test]
    fn same_text_seals_differently() {
        let key = [7u8; 32];
        assert_ne!(seal(&key, "a", "x"), seal(&key, "a", "x"));
    }

    #[test]
    fn rejects_wrong_key_account_or_tampering() {
        let key = [7u8; 32];
        let sealed = seal(&key, "acc1", "secret");
        assert_eq!(open(&[8u8; 32], "acc1", &sealed), None);
        assert_eq!(open(&key, "acc2", &sealed), None);
        let mut bytes = B64.decode(sealed.strip_prefix(PREFIX).unwrap()).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        assert_eq!(open(&key, "acc1", &format!("{PREFIX}{}", B64.encode(bytes))), None);
        assert_eq!(open(&key, "acc1", "not sealed"), None);
        assert_eq!(open(&key, "acc1", "v1:%%%"), None);
    }

    #[test]
    fn handles_long_tokens() {
        let key = [1u8; 32];
        let long = "t".repeat(20_000);
        assert_eq!(open(&key, "a", &seal(&key, "a", &long)).as_deref(), Some(long.as_str()));
    }

    /// Uses the real OS password store with a throwaway entry, then deletes it:
    /// cargo test --lib -- --ignored os_password_store
    #[test]
    #[ignore]
    fn os_password_store() {
        let name = format!("test-key-{}", std::process::id());
        let first = load_or_create(&name).expect("create");
        let second = load_or_create(&name).expect("read back");
        assert_eq!(first, second, "the same key comes back");
        keyring::Entry::new(SERVICE, &name).unwrap().delete_credential().unwrap();
        assert!(matches!(keyring::Entry::new(SERVICE, &name).unwrap().get_password(), Err(keyring::Error::NoEntry)));
    }
}
