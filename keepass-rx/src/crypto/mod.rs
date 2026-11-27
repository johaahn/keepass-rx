use anyhow::{Result, anyhow};
#[cfg(feature = "sailfish")]
use atomic_write_file::AtomicWriteFile;
use crypto_kdf::Key as KdfKey;
#[cfg(not(feature = "sailfish"))]
use keyring::Entry as KeyringEntry;
use libsodium_rs::crypto_aead::xchacha20poly1305;
use libsodium_rs::crypto_aead::xchacha20poly1305::{Key as ChaChaKey, Nonce};
use libsodium_rs::crypto_kdf;
#[cfg(not(feature = "sailfish"))]
use libsodium_rs::crypto_pwhash;
use libsodium_rs::random;
use libsodium_rs::utils::{SecureVec, vec_utils};
use poison_guard::Poison;
use secstr::SecUtf8;
#[cfg(feature = "sailfish")]
use secstr::SecVec;
#[cfg(feature = "sailfish")]
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
#[cfg(feature = "sailfish")]
use std::os::unix::fs::OpenOptionsExt;
#[cfg(feature = "sailfish")]
use std::path::{Path, PathBuf};
#[cfg(feature = "sailfish")]
use std::time::{Duration, SystemTime};
use uuid::Uuid;
use zeroize::Zeroize;

#[cfg(feature = "sailfish")]
pub mod sailfish_secrets;
#[cfg(any(test, feature = "test-keyring"))]
pub mod test_keyring;

#[derive(Debug, thiserror::Error)]
#[error("Incorrect unlock code")]
pub struct IncorrectShortCode;

const SHORT_PW_LENGTH: usize = 5;
const KERNEL_SECRET_USER: &str = "keepassrx";
#[cfg(feature = "sailfish")]
const SAVED_PASSWORD_LIFETIME: Duration = Duration::from_secs(8 * 60 * 60);

pub fn to_short_password(value: &str) -> Result<&str> {
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

#[cfg(not(feature = "sailfish"))]
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

/// Stashes a secret out of the app's normal heap. On Ubuntu Touch this uses the
/// kernel keyring. On Sailfish OS the keyring syscalls (add_key/keyctl) are
/// blocked by the Sailjail seccomp filter, so it holds the secret in libsodium
/// secure memory (mlock'd, guard pages, zeroized on drop) instead.
#[cfg(not(feature = "sailfish"))]
pub struct KernelBackedSecret {
    entry_id: Poison<String>,
}

#[cfg(not(feature = "sailfish"))]
impl Clone for KernelBackedSecret {
    fn clone(&self) -> Self {
        KernelBackedSecret {
            entry_id: Poison::new(
                self.entry_id
                    .get()
                    .expect("Cannot clone poisoned encrypted password")
                    .clone(),
            ),
        }
    }
}

#[cfg(not(feature = "sailfish"))]
#[allow(dead_code)]
impl KernelBackedSecret {
    fn entry_for(service: &str) -> Result<KeyringEntry> {
        Ok(KeyringEntry::new(service, KERNEL_SECRET_USER)?)
    }

    pub fn new_with_id(id: &str, pw: &[u8]) -> Result<Self> {
        let entry = Self::entry_for(id)?;
        entry.set_secret(pw)?;

        Ok(Self {
            entry_id: Poison::new(id.to_owned()),
        })
    }

    pub fn new(pw: &[u8]) -> Result<Self> {
        Self::new_with_id(&Uuid::new_v4().to_string(), pw)
    }

    pub fn retrieve(&self) -> Result<Vec<u8>> {
        let service = self
            .entry_id
            .get()
            .map_err(|poisoned| anyhow!("{poisoned}"))?;
        let keyring_entry = Self::entry_for(service.as_str())?;

        Ok(keyring_entry.get_secret()?)
    }

    pub fn poison(&mut self) -> Result<()> {
        let service_guard = Poison::unless_recovered(&mut self.entry_id)
            .map_err(|poisoned| poisoned.into_error())?;
        let keyring_entry = Self::entry_for(service_guard.as_str())?;

        keyring_entry.delete_credential()?;
        Ok(())
    }

    pub fn is_poisoned(&self) -> bool {
        self.entry_id.is_poisoned()
    }
}

#[cfg(feature = "sailfish")]
pub struct KernelBackedSecret {
    secret: Poison<SecVec<u8>>,
}

#[cfg(feature = "sailfish")]
impl Clone for KernelBackedSecret {
    fn clone(&self) -> Self {
        let existing = self
            .secret
            .get()
            .expect("Cannot clone poisoned encrypted password");
        KernelBackedSecret {
            secret: Poison::new(existing.clone()),
        }
    }
}

#[cfg(feature = "sailfish")]
#[allow(dead_code)]
impl KernelBackedSecret {
    pub fn new_with_id(_id: &str, pw: &[u8]) -> Result<Self> {
        Ok(Self {
            secret: Poison::new(SecVec::new(pw.to_vec())),
        })
    }

    pub fn new(pw: &[u8]) -> Result<Self> {
        Self::new_with_id(KERNEL_SECRET_USER, pw)
    }

    pub fn retrieve(&self) -> Result<Vec<u8>> {
        let secret = self
            .secret
            .get()
            .map_err(|poisoned| anyhow!("{poisoned}"))?;
        Ok(secret.unsecure().to_vec())
    }

    pub fn poison(&mut self) -> Result<()> {
        // Consume the guard without recovering: the Poison is left poisoned and
        // the SecureVec is dropped (zeroizing its memory).
        Poison::unless_recovered(&mut self.secret)
            .map_err(|poisoned| poisoned.into_error())?;
        Ok(())
    }

    pub fn is_poisoned(&self) -> bool {
        self.secret.is_poisoned()
    }
}

impl Zeroize for KernelBackedSecret {
    fn zeroize(&mut self) {
        let _ = self.poison();
    }
}

/// Holds the encrypted database password and its associated
/// cryptographic information. The salt and nonce are stored in
/// memory, while the encrypted password is passed off to the kernel
/// keyring service. Ideally, the nonce would also be given to the
/// kernel keyring service, but trait bounds seem tricky.
#[derive(Zeroize, Clone)]
pub struct EncryptedPassword {
    #[cfg(not(feature = "sailfish"))]
    pw_salt: Vec<u8>,

    secret: KernelBackedSecret,
    #[cfg(not(feature = "sailfish"))]
    #[zeroize(skip)]
    nonce: Nonce,
    #[cfg(feature = "sailfish")]
    #[zeroize(skip)]
    iv: [u8; sailfish_secrets::IV_LEN],
}

#[cfg(not(feature = "sailfish"))]
impl EncryptedPassword {
    fn seal(db_password: &str, key: &ChaChaKey) -> Result<Self> {
        let nonce = Nonce::generate();
        let ciphertext =
            xchacha20poly1305::encrypt(db_password.as_bytes(), None, &nonce, key)?;

        Ok(EncryptedPassword {
            pw_salt: Vec::new(),
            secret: KernelBackedSecret::new(&ciphertext)?,
            nonce,
        })
    }

    fn open(&self, key: &ChaChaKey) -> Result<SecUtf8> {
        let encrypted_password = self.secret.retrieve()?;

        let master_password =
            xchacha20poly1305::decrypt(&encrypted_password, None, &self.nonce, key)
                .map(|master_pw| std::str::from_utf8(master_pw.as_slice()).map(SecUtf8::from))
                .map_err(|_| IncorrectShortCode)??;

        Ok(master_password)
    }

    pub fn new(db_password: SecUtf8) -> Result<Self> {
        let db_password = db_password.unsecure();
        let short_password = to_short_password(db_password)?;

        let mut pw_salt = [0u8; crypto_pwhash::SALTBYTES];
        random::fill_bytes(&mut pw_salt);

        let key_raw_bytes = hash_password(short_password, &pw_salt)?;
        let key = ChaChaKey::from_bytes(&key_raw_bytes)?;

        Ok(EncryptedPassword {
            pw_salt: pw_salt.to_vec(),
            ..Self::seal(db_password, &key)?
        })
    }

    pub fn reveal(&self, short_password: SecUtf8) -> Result<SecUtf8> {
        let key_raw_bytes = hash_password(short_password.unsecure(), &self.pw_salt)?;
        drop(short_password);
        self.open(&ChaChaKey::from_bytes(&key_raw_bytes)?)
    }

    pub fn decrypt(mut self, short_password: SecUtf8) -> Result<SecUtf8> {
        let master_password = self.reveal(short_password)?;
        let _ = self.secret.poison(); // nuke the key in KRS
        Ok(master_password)
    }
}

#[cfg(feature = "sailfish")]
impl EncryptedPassword {
    pub fn new(db_password: SecUtf8) -> Result<Self> {
        let mut iv = [0u8; sailfish_secrets::IV_LEN];
        random::fill_bytes(&mut iv);
        sailfish_secrets::create_key()?;
        let ciphertext = sailfish_secrets::encrypt(db_password.unsecure().as_bytes(), &iv)?;

        Ok(EncryptedPassword {
            secret: KernelBackedSecret::new(&ciphertext)?,
            iv,
        })
    }

    pub fn reveal(&self) -> Result<SecUtf8> {
        let sealed = self.secret.retrieve()?;
        let master_password = sailfish_secrets::decrypt(&sealed, &self.iv)?;
        Ok(SecUtf8::from(str::from_utf8(&master_password)?))
    }

    pub fn decrypt(mut self) -> Result<SecUtf8> {
        let master_password = self.reveal()?;
        let _ = self.secret.poison();
        Ok(master_password)
    }

    pub fn save(&self, database: &Path) -> Result<()> {
        let saved = SavedPassword {
            database: database.to_path_buf(),
            ciphertext: self.secret.retrieve()?,
            iv: self.iv,
            written: SystemTime::now(),
        };
        let mut file = AtomicWriteFile::options()
            .mode(0o600)
            .open(saved_password_path()?)?;
        serde_json::to_writer(&mut file, &saved)?;
        file.commit()?;
        Ok(())
    }

    /// Forgets an expired saved password, or one saved for another database.
    pub fn load(database: &Path) -> Result<Option<Self>> {
        let saved: SavedPassword = match std::fs::read(saved_password_path()?) {
            Ok(bytes) => serde_json::from_slice(&bytes)?,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err.into()),
        };

        let fresh = saved
            .written
            .elapsed()
            .is_ok_and(|age| age <= SAVED_PASSWORD_LIFETIME);
        if !fresh || saved.database != database {
            Self::forget_saved();
            return Ok(None);
        }

        Ok(Some(EncryptedPassword {
            secret: KernelBackedSecret::new(&saved.ciphertext)?,
            iv: saved.iv,
        }))
    }

    pub fn forget_saved() {
        if let Ok(path) = saved_password_path()
            && let Err(err) = std::fs::remove_file(path)
            && err.kind() != std::io::ErrorKind::NotFound
        {
            log::warn!("Could not remove the saved master password: {err}");
        }
        if let Err(err) = sailfish_secrets::delete() {
            log::warn!("Could not delete the unlock key from Sailfish Secrets: {err}");
        }
    }
}

#[cfg(feature = "sailfish")]
#[derive(Serialize, Deserialize)]
struct SavedPassword {
    database: PathBuf,
    ciphertext: Vec<u8>,
    iv: [u8; sailfish_secrets::IV_LEN],
    written: SystemTime,
}

#[cfg(feature = "sailfish")]
fn saved_password_path() -> Result<PathBuf> {
    let runtime_dir = dirs::runtime_dir().ok_or(anyhow!("No runtime directory"))?;
    Ok(runtime_dir.join("harbour-keepassrx").join("unlock"))
}

// Must be exactly 8 bytes
const CONTEXT: &[u8; 8] = b"KEEPSSRX";

pub struct MasterKey {
    key: RefCell<KernelBackedSecret>,
}

impl MasterKey {
    pub fn new() -> Result<Self> {
        let master_key = crypto_kdf::Key::generate()?;
        let secret = KernelBackedSecret::new_with_id(
            &Uuid::new_v4().to_string(),
            master_key.as_bytes(),
        )?;
        Ok(Self {
            key: RefCell::new(secret),
        })
    }

    fn get_kdf_key(&self) -> Result<KdfKey> {
        let raw_bytes = &self.key.borrow_mut().retrieve()?;
        let key = KdfKey::from_slice(&raw_bytes)?;
        Ok(key)
    }

    pub fn derive_subkey(&self, id: u64) -> Result<SecureVec<u8>> {
        let mut subkey = crypto_kdf::derive_from_key(32, id, CONTEXT, &self.get_kdf_key()?)?;
        let mut secure_subkey = vec_utils::secure_vec(subkey.len())?;

        secure_subkey.copy_from_slice(&subkey);
        subkey.zeroize();

        Ok(secure_subkey)
    }

    pub fn poison(&self) -> Result<()> {
        self.key.borrow_mut().poison()?;
        Ok(())
    }
}

impl Zeroize for MasterKey {
    fn zeroize(&mut self) {
        let _ = self.poison();
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
        self.value.borrow_mut().zeroize();
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
        self.nonce.replace(new_value.nonce.into_inner());
        self.value.replace(new_value.value.into_inner());

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

    #[test]
    fn reveal_returns_password_and_keeps_it_decryptable() {
        test_keyring::install();

        let encrypted =
            EncryptedPassword::new(SecUtf8::from("12345678910")).expect("Encryption failed");

        let first = encrypted
            .reveal(SecUtf8::from("12345"))
            .expect("First reveal failed");
        assert_eq!(first.unsecure(), "12345678910");

        let second = encrypted
            .reveal(SecUtf8::from("12345"))
            .expect("Second reveal failed");
        assert_eq!(second.unsecure(), "12345678910");

        let decrypted = encrypted
            .decrypt(SecUtf8::from("12345"))
            .expect("Decryption failed");
        assert_eq!(decrypted.unsecure(), "12345678910");
    }

    #[test]
    fn reveal_with_wrong_code_errors() {
        test_keyring::install();

        let encrypted =
            EncryptedPassword::new(SecUtf8::from("12345678910")).expect("Encryption failed");

        let err = encrypted
            .reveal(SecUtf8::from("99999"))
            .expect_err("Wrong code was accepted");
        assert!(err.downcast_ref::<IncorrectShortCode>().is_some());
        assert!(encrypted.reveal(SecUtf8::from("12345")).is_ok());
    }

    #[test]
    fn random_key_round_trip() {
        test_keyring::install();

        let key = ChaChaKey::generate();
        let encrypted =
            EncryptedPassword::seal("12345678910", &key).expect("Encryption failed");

        let revealed = encrypted.open(&key).expect("Decryption failed");
        assert_eq!(revealed.unsecure(), "12345678910");

        let err = encrypted
            .open(&ChaChaKey::generate())
            .expect_err("Wrong key was accepted");
        assert!(err.downcast_ref::<IncorrectShortCode>().is_some());
    }

    #[test]
    fn encrypt_and_decrypt() {
        test_keyring::install();

        let easy =
            EncryptedPassword::new(SecUtf8::from("12345678910")).expect("Encryption failed");
        let decrypted = easy
            .decrypt(SecUtf8::from("12345"))
            .expect("Decryption failed");

        assert_eq!(decrypted.unsecure(), "12345678910");
    }
}
