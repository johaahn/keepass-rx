use anyhow::{Result, anyhow};
use keyring::Entry as KeyringEntry;
use libsodium_rs::crypto_aead::xchacha20poly1305;
use libsodium_rs::crypto_aead::xchacha20poly1305::{Key, Nonce};
use libsodium_rs::crypto_kdf;
use libsodium_rs::crypto_pwhash;
use libsodium_rs::random;
use secstr::SecUtf8;
use std::sync::Arc;
use zeroize::Zeroize;

const SHORT_PW_LENGTH: usize = 5;

fn to_short_password(value: &str) -> Result<&str> {
    let short_password = {
        if value.len() > SHORT_PW_LENGTH {
            let split_pos = value.char_indices().nth(SHORT_PW_LENGTH).unwrap().0;
            &value[..split_pos]
        } else {
            &value
        }
    };

    Ok(short_password)
}

fn hash_password(value: &str, pw_salt: &[u8]) -> Result<Vec<u8>> {
    let pw_hash = crypto_pwhash::pwhash(
        32,
        value.as_bytes(),
        pw_salt,
        crypto_pwhash::OPSLIMIT_INTERACTIVE,
        crypto_pwhash::MEMLIMIT_INTERACTIVE,
        crypto_pwhash::ALG_DEFAULT,
    )?;

    Ok(pw_hash)
}

/// Simple wrapper around the keyring library to store credentials in
/// a secure(ish?) place.
#[derive(Clone)]
pub struct KernelBackedSecret {
    secret: Arc<KeyringEntry>,
}

#[allow(dead_code)]
impl KernelBackedSecret {
    pub fn new(pw: Vec<u8>) -> Result<Self> {
        let entry = KeyringEntry::new("keepassrx", "keepassrx")?;
        entry.set_secret(&pw.as_ref())?;
        Ok(Self {
            secret: Arc::new(entry),
        })
    }

    pub fn retrieve(&self) -> Result<Vec<u8>> {
        let value = self.secret.get_secret()?;
        Ok(value.into())
    }

    pub fn destroy(self) -> Result<()> {
        Ok(self.secret.delete_credential()?)
    }
}

impl Zeroize for KernelBackedSecret {
    fn zeroize(&mut self) {
        let _ = self.secret.delete_credential();
    }
}

/// Holds the encrypted database password and its associated
/// cryptographic information. The salt and nonce are stored in
/// memory, while the encrypted password is passed off to the kernel
/// keyring service. Ideally, the nonce would also be given to the
/// kernel keyring service, but trait bounds seem tricky.
#[derive(Zeroize, Clone)]
pub struct EncryptedPassword {
    pw_salt: Vec<u8>,

    secret: KernelBackedSecret,
    #[zeroize(skip)]
    nonce: Nonce,
}

impl EncryptedPassword {
    pub fn new(db_password: SecUtf8) -> Result<Self> {
        let db_password = db_password.unsecure();
        let short_password = to_short_password(db_password)?;

        let mut pw_salt = [0u8; crypto_pwhash::SALTBYTES];
        random::fill_bytes(&mut pw_salt);

        let key_raw_bytes = hash_password(short_password, &pw_salt)?;
        let key = Key::from_bytes(&key_raw_bytes)?;
        let nonce = Nonce::generate();

        let ciphertext =
            xchacha20poly1305::encrypt(db_password.as_bytes(), None, &nonce, &key)?;

        Ok(EncryptedPassword {
            pw_salt: pw_salt.to_vec(),
            secret: KernelBackedSecret::new(ciphertext)?,
            nonce,
        })
    }

    pub fn decrypt(self, short_password: SecUtf8) -> Result<SecUtf8> {
        let encrypted_password = self.secret.retrieve()?;
        let key_raw_bytes = hash_password(short_password.unsecure(), &self.pw_salt)?;
        drop(short_password);
        let key = Key::from_bytes(&key_raw_bytes)?;

        let master_password =
            xchacha20poly1305::decrypt(&encrypted_password, None, &self.nonce, &key)
                .map(|master_pw| {
                    std::str::from_utf8(master_pw.as_slice())
                        .map(|pw_utf8| SecUtf8::from(pw_utf8))
                })
                .map_err(|err| anyhow!(err))??;

        Ok(master_password)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortens_long_password() {
        let short_pw = to_short_password("VeryLongPassword").expect("shortening failed");
        assert_eq!(short_pw, "VeryL");
    }

    #[test]
    fn shortens_exact_password() {
        let short_pw = to_short_password("VeryL").expect("shortening failed");
        assert_eq!(short_pw, "VeryL");
    }

    #[test]
    fn shortens_short_password() {
        let short_pw = to_short_password("Ver").expect("shortening failed");
        assert_eq!(short_pw, "Ver");
    }

    #[test]
    fn shortens_empty_password() {
        let short_pw = to_short_password("").expect("shortening failed");
        assert_eq!(short_pw, "");
    }

    // #[test]
    // fn encrypt_and_decrypt() {
    //     // TODO this fails for some reason.
    //     let easy = EncryptedPassword::new("12345678910".into()).expect("Encryption failed");
    //     let decrypted = easy
    //         .decrypt(&SecUtf8::from("1234"))
    //         .expect("Decryption failed");

    //     assert_eq!(decrypted.unsecure(), "12345678910");
    // }
}
