use anyhow::{Result, anyhow};
use crypto_kdf::Key as KdfKey;
use keyring::Entry as KeyringEntry;
use libsodium_rs::crypto_aead::xchacha20poly1305;
use libsodium_rs::crypto_aead::xchacha20poly1305::{Key as ChaChaKey, Nonce};
use libsodium_rs::crypto_kdf;
use libsodium_rs::crypto_pwhash;
use libsodium_rs::random;
use libsodium_rs::utils::{SecureVec, vec_utils};
use secstr::SecUtf8;
use std::cell::RefCell;
use std::sync::Arc;
use uuid::Uuid;
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
        let key = ChaChaKey::from_bytes(&key_raw_bytes)?;
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
        let key = ChaChaKey::from_bytes(&key_raw_bytes)?;

        let master_password =
            xchacha20poly1305::decrypt(&encrypted_password, None, &self.nonce, &key)
                .map(|master_pw| {
                    std::str::from_utf8(master_pw.as_slice())
                        .map(|pw_utf8| SecUtf8::from(pw_utf8))
                })
                .map_err(|err| anyhow!(err))??;

        let _ = self.secret.destroy(); // nuke the key in KRS
        Ok(master_password)
    }
}

// Must be exactly 8 bytes
const CONTEXT: &[u8; 8] = b"KEEPSSRX";

#[derive(Clone)]
pub struct MasterKey {
    key: KdfKey,
}

impl MasterKey {
    pub fn new() -> Result<Self> {
        let master_key = crypto_kdf::Key::generate()?;
        Ok(Self { key: master_key })
    }

    pub fn derive_subkey(&self, id: u64) -> Result<Vec<u8>> {
        let subkey = crypto_kdf::derive_from_key(32, id, CONTEXT, &self.key)?;
        Ok(subkey)
    }
}

#[derive(Clone)]
pub struct EncryptedValue {
    id: u64,
    value: RefCell<Vec<u8>>,
    nonce: RefCell<Nonce>,
}

impl Zeroize for EncryptedValue {
    fn zeroize(&mut self) {
        let mut this_nonce = self.nonce.swap(&RefCell::new(Nonce::generate()));
        let mut this_value = self.value.take();

        this_nonce.zeroize();
        this_value.zeroize();
    }
}

impl EncryptedValue {
    pub fn new(key: &MasterKey, id: u64, value: SecureVec<u8>) -> Result<Self> {
        let subkey = key.derive_subkey(id)?;
        let encryption_key = ChaChaKey::from_bytes(&subkey)?;
        let nonce = Nonce::generate();

        let ciphertext = xchacha20poly1305::encrypt(&value, None, &nonce, &encryption_key)?;
        drop(value);

        Ok(Self {
            value: RefCell::new(ciphertext),
            nonce: RefCell::new(nonce),
            id: id,
        })
    }

    fn decrypt(&self, key: &MasterKey) -> Result<SecureVec<u8>> {
        let subkey = key.derive_subkey(self.id)?;
        let encryption_key = ChaChaKey::from_bytes(&subkey)?;

        let mut plaintext = xchacha20poly1305::decrypt(
            &self.value.borrow(),
            None,
            &self.nonce.borrow(),
            &encryption_key,
        )?;

        let mut secure_plaintext = vec_utils::secure_vec::<u8>(plaintext.len())?;
        secure_plaintext.copy_from_slice(plaintext.as_slice());
        plaintext.zeroize();

        Ok(secure_plaintext)
    }

    fn reencrypt(&self, key: &MasterKey) -> Result<()> {
        let decrypted = self.decrypt(key)?;

        let mut value_mut = self.value.borrow_mut();
        value_mut.zeroize();
        drop(value_mut);

        let new_value = Self::new(key, self.id, decrypted)?;
        self.nonce.swap(&new_value.nonce);
        self.value.swap(&new_value.value);

        Ok(())
    }

    pub fn expose(&self, key: &MasterKey) -> Result<SecureVec<u8>> {
        let plaintext_value = self.decrypt(key)?;
        self.reencrypt(key)?;
        Ok(plaintext_value)
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
