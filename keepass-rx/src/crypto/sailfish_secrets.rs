use std::collections::HashMap;

use anyhow::{Context, Result, anyhow};
use serde::Serialize;
use serde::de::DeserializeOwned;
use zbus::blocking::{Connection, connection::Builder};
use zbus::zvariant::{DynamicType, OwnedValue, Type, Value};
use zeroize::Zeroizing;

use super::IncorrectShortCode;

const DISCOVERY_DESTINATION: &str = "org.sailfishos.secrets.daemon.discovery";
const DISCOVERY_PATH: &str = "/Sailfish/Secrets/Discovery";
const SECRETS_PATH: &str = "/Sailfish/Secrets";
const SECRETS_INTERFACE: &str = "org.sailfishos.secrets";
const CRYPTO_PATH: &str = "/Sailfish/Crypto";
const CRYPTO_INTERFACE: &str = "org.sailfishos.crypto";
const COLLECTION: &str = "keepassrx";
const KEY_NAME: &str = "unlock-key";
const PEER_TO_PEER_ADDRESS: &str = "peerToPeerAddress";
const FIND_SECRETS: &str = "findSecrets";
const CREATE_COLLECTION: &str = "createCollection";
const GENERATE_STORED_KEY: &str = "generateStoredKey";
const ENCRYPT: &str = "encrypt";
const DECRYPT: &str = "decrypt";
const DELETE_COLLECTION: &str = "deleteCollection";
const STORAGE_PLUGIN: &str = "org.sailfishos.secrets.plugin.storage.sqlite";
const ENCRYPTION_PLUGIN: &str = "org.sailfishos.secrets.plugin.encryption.openssl";
const CRYPTO_PLUGIN: &str = "org.sailfishos.crypto.plugin.crypto.openssl";
const DEVICE_LOCK_KEEP_UNLOCKED: i32 = 0;
const OWNER_ONLY: i32 = 0;
const PREVENT_INTERACTION: i32 = 0;
const OPERATOR_OR: i32 = 0;
const RESULT_SUCCEEDED: i32 = 0;
const ALGORITHM_AES: i32 = 260;
const OPERATIONS_ENCRYPT_DECRYPT: i32 = 8 | 16;
const COMPONENTS_METADATA: i32 = 1;
const KEY_BITS: i32 = 256;
const BLOCK_MODE_GCM: i32 = 10;
const PADDING_NONE: i32 = 2;
const VERIFICATION_SUCCEEDED: i32 = 1;
const PLUGIN_AVAILABLE: i32 = 1;
const GET_PLUGIN_INFO: &str = "getPluginInfo";
const TAG_LEN: usize = 16;
const AUTHENTICATION_DATA: &[u8] = b"keepassrx-unlock";
pub const IV_LEN: usize = 12;

type Identifier<'a> = (&'a str, &'a str, &'a str);
type Parameters<'a> = HashMap<String, Value<'a>>;
type Key<'a> = (
    Identifier<'a>,
    i32,
    i32,
    i32,
    i32,
    i32,
    Vec<u8>,
    Vec<u8>,
    Vec<u8>,
    Vec<Vec<u8>>,
    (Parameters<'a>,),
);
type OwnedKey = (
    (String, String, String),
    i32,
    i32,
    i32,
    i32,
    i32,
    Vec<u8>,
    Vec<u8>,
    Vec<u8>,
    Vec<Vec<u8>>,
    (HashMap<String, OwnedValue>,),
);
type KeyPairParameters<'a> = (i32, Parameters<'a>, Parameters<'a>);
type KeyDerivationParameters<'a> = (
    Vec<u8>,
    Vec<u8>,
    (i32,),
    (i32,),
    (i32,),
    (i32,),
    i64,
    i32,
    i32,
    i32,
    Parameters<'a>,
);
type InteractionParameters = (
    String,
    String,
    String,
    String,
    (i32,),
    String,
    HashMap<i32, String>,
    (i32,),
    (i32,),
);
type SecretsResult = (i32, i32, String);
type CryptoResult = (i32, i32, i32, String);
type PluginInfo = (String, String, i32, i32);
type SecretsPluginsReply = (
    SecretsResult,
    Vec<PluginInfo>,
    Vec<PluginInfo>,
    Vec<PluginInfo>,
    Vec<PluginInfo>,
);
type CryptoPluginsReply = (CryptoResult, Vec<PluginInfo>, Vec<PluginInfo>);
type CreateCollectionCall<'a> = (&'a str, &'a str, &'a str, (i32,), (i32,));
type DeleteCollectionCall<'a> = (&'a str, &'a str, (i32,), &'a str);
type FindSecretsCall<'a> = (
    &'a str,
    &'a str,
    HashMap<&'a str, &'a str>,
    (i32,),
    (i32,),
    &'a str,
);
type FindSecretsReply = (SecretsResult, Vec<(String, String, String)>);
type GenerateStoredKeyCall<'a> = (
    Key<'a>,
    KeyPairParameters<'a>,
    KeyDerivationParameters<'a>,
    InteractionParameters,
    Parameters<'a>,
    &'a str,
);
type GenerateStoredKeyReply = (CryptoResult, OwnedKey);
type EncryptCall<'a> = (
    &'a [u8],
    &'a [u8],
    Key<'a>,
    (i32,),
    (i32,),
    &'a [u8],
    Parameters<'a>,
    &'a str,
);
type EncryptReply = (CryptoResult, Vec<u8>, Vec<u8>);
type DecryptCall<'a> = (
    &'a [u8],
    &'a [u8],
    Key<'a>,
    (i32,),
    (i32,),
    &'a [u8],
    &'a [u8],
    Parameters<'a>,
    &'a str,
);
type DecryptReply = (CryptoResult, Vec<u8>, (i32,));

fn connect() -> Result<Connection> {
    let session = Connection::session()?;
    let reply = session.call_method(
        Some(DISCOVERY_DESTINATION),
        DISCOVERY_PATH,
        Some(DISCOVERY_DESTINATION),
        PEER_TO_PEER_ADDRESS,
        &(),
    )?;
    let address: String = reply.body().deserialize()?;

    Ok(Builder::address(address.as_str())?.p2p().build()?)
}

fn call<B, R>(path: &str, interface: &str, method: &str, body: &B) -> Result<R>
where
    B: Serialize + DynamicType,
    R: DeserializeOwned + Type,
{
    let reply = connect()?.call_method(None::<&str>, path, Some(interface), method, body)?;
    Ok(reply.body().deserialize()?)
}

fn secrets<B, R>(method: &str, body: &B) -> Result<R>
where
    B: Serialize + DynamicType,
    R: DeserializeOwned + Type,
{
    call(SECRETS_PATH, SECRETS_INTERFACE, method, body)
        .with_context(|| format!("Sailfish Secrets {method} failed"))
}

fn crypto<B, R>(method: &str, body: &B) -> Result<R>
where
    B: Serialize + DynamicType,
    R: DeserializeOwned + Type,
{
    call(CRYPTO_PATH, CRYPTO_INTERFACE, method, body)
        .with_context(|| format!("Sailfish Crypto {method} failed"))
}

fn check(method: &str, result: SecretsResult) -> Result<()> {
    let (code, _, message) = result;
    if code == RESULT_SUCCEEDED {
        Ok(())
    } else {
        Err(anyhow!("Sailfish Secrets {method} failed: {message}"))
    }
}

fn check_crypto(method: &str, result: CryptoResult) -> Result<()> {
    let (code, _, _, message) = result;
    if code == RESULT_SUCCEEDED {
        Ok(())
    } else {
        Err(anyhow!("Sailfish Crypto {method} failed: {message}"))
    }
}

fn key(algorithm: i32, operations: i32, components: i32, bits: i32) -> Key<'static> {
    (
        (KEY_NAME, COLLECTION, STORAGE_PLUGIN),
        0,
        algorithm,
        operations,
        components,
        bits,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        (HashMap::new(),),
    )
}

fn key_reference() -> Key<'static> {
    key(0, 0, 0, 0)
}

fn require_unlocked() -> Result<()> {
    let call: FindSecretsCall = (
        COLLECTION,
        STORAGE_PLUGIN,
        HashMap::from([("Type", "CryptoKey")]),
        (OPERATOR_OR,),
        (PREVENT_INTERACTION,),
        "",
    );
    let (result, identifiers): FindSecretsReply = secrets(FIND_SECRETS, &call)?;
    check(FIND_SECRETS, result)?;

    if identifiers.iter().any(|(name, _, _)| name == KEY_NAME) {
        Ok(())
    } else {
        Err(anyhow!("The unlock key is missing from Sailfish Secrets"))
    }
}

pub fn create_key() -> Result<()> {
    let _ = delete();

    let call: CreateCollectionCall = (
        COLLECTION,
        STORAGE_PLUGIN,
        ENCRYPTION_PLUGIN,
        (DEVICE_LOCK_KEEP_UNLOCKED,),
        (OWNER_ONLY,),
    );
    check(CREATE_COLLECTION, secrets(CREATE_COLLECTION, &call)?)?;

    let call: GenerateStoredKeyCall = (
        key(
            ALGORITHM_AES,
            OPERATIONS_ENCRYPT_DECRYPT,
            COMPONENTS_METADATA,
            KEY_BITS,
        ),
        (0, HashMap::new(), HashMap::new()),
        (
            Vec::new(),
            Vec::new(),
            (0,),
            (0,),
            (0,),
            (0,),
            0,
            0,
            0,
            0,
            HashMap::new(),
        ),
        InteractionParameters::default(),
        HashMap::new(),
        CRYPTO_PLUGIN,
    );
    let (result, _): GenerateStoredKeyReply = crypto(GENERATE_STORED_KEY, &call)?;
    check_crypto(GENERATE_STORED_KEY, result)
}

pub fn encrypt(plaintext: &[u8], iv: &[u8; IV_LEN]) -> Result<Vec<u8>> {
    require_unlocked()?;

    let call: EncryptCall = (
        plaintext,
        iv,
        key_reference(),
        (BLOCK_MODE_GCM,),
        (PADDING_NONE,),
        AUTHENTICATION_DATA,
        HashMap::new(),
        CRYPTO_PLUGIN,
    );
    let (result, mut ciphertext, tag): EncryptReply = crypto(ENCRYPT, &call)?;
    check_crypto(ENCRYPT, result)?;

    ciphertext.extend_from_slice(&tag);
    Ok(ciphertext)
}

pub fn decrypt(sealed: &[u8], iv: &[u8; IV_LEN]) -> Result<Zeroizing<Vec<u8>>> {
    let split = sealed
        .len()
        .checked_sub(TAG_LEN)
        .ok_or(anyhow!("The sealed master password is too short"))?;
    let (ciphertext, tag) = sealed.split_at(split);

    require_unlocked()?;

    let call: DecryptCall = (
        ciphertext,
        iv,
        key_reference(),
        (BLOCK_MODE_GCM,),
        (PADDING_NONE,),
        AUTHENTICATION_DATA,
        tag,
        HashMap::new(),
        CRYPTO_PLUGIN,
    );
    let (result, plaintext, (verification,)): DecryptReply = crypto(DECRYPT, &call)?;
    let plaintext = Zeroizing::new(plaintext);
    check_crypto(DECRYPT, result)?;

    if verification == VERIFICATION_SUCCEEDED {
        Ok(plaintext)
    } else {
        Err(IncorrectShortCode.into())
    }
}

pub fn delete() -> Result<()> {
    let call: DeleteCollectionCall = (COLLECTION, STORAGE_PLUGIN, (PREVENT_INTERACTION,), "");
    check(DELETE_COLLECTION, secrets(DELETE_COLLECTION, &call)?)
}

fn require_plugin(plugins: &[PluginInfo], name: &str) -> Result<()> {
    let available = plugins.iter().any(|(_, plugin, _, status)| {
        plugin == name && status & PLUGIN_AVAILABLE == PLUGIN_AVAILABLE
    });
    if available {
        Ok(())
    } else {
        Err(anyhow!(
            "The Sailfish Secrets plugin {name} is not available"
        ))
    }
}

pub fn probe() -> Result<()> {
    let (result, storage, encryption, _, _): SecretsPluginsReply =
        secrets(GET_PLUGIN_INFO, &())?;
    check(GET_PLUGIN_INFO, result)?;
    let (result, crypto_plugins, _): CryptoPluginsReply = crypto(GET_PLUGIN_INFO, &())?;
    check_crypto(GET_PLUGIN_INFO, result)?;

    require_plugin(&storage, STORAGE_PLUGIN)?;
    require_plugin(&encryption, ENCRYPTION_PLUGIN)?;
    require_plugin(&crypto_plugins, CRYPTO_PLUGIN)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signature<T: Type>() -> String {
        T::SIGNATURE.to_string()
    }

    const SECRETS_ADAPTOR: &str = "sailfish-secrets 5a8d33e daemon/SecretsImpl/secrets_p.h";
    const CRYPTO_ADAPTOR: &str = "sailfish-secrets 5a8d33e daemon/CryptoImpl/crypto_p.h";
    const KEY: &str = "((sss)iiiiiayayayaay(a{sv}))";

    #[test]
    fn create_collection_matches_secrets_p_h_91() {
        assert_eq!(
            signature::<CreateCollectionCall>(),
            "(sss(i)(i))",
            "{SECRETS_ADAPTOR}:91"
        );
        assert_eq!(
            signature::<SecretsResult>(),
            "(iis)",
            "{SECRETS_ADAPTOR}:97"
        );
    }

    #[test]
    fn delete_collection_matches_secrets_p_h_117() {
        assert_eq!(
            signature::<DeleteCollectionCall>(),
            "(ss(i)s)",
            "{SECRETS_ADAPTOR}:117"
        );
    }

    #[test]
    fn find_secrets_matches_secrets_p_h_181_and_secrets_cpp_1677() {
        assert_eq!(
            signature::<FindSecretsCall>(),
            "(ssa{ss}(i)(i)s)",
            "{SECRETS_ADAPTOR}:181"
        );
        assert_eq!(
            signature::<FindSecretsReply>(),
            "((iis)a(sss))",
            "daemon/SecretsImpl/secrets.cpp:1677 sends QVector<Secret::Identifier>"
        );
    }

    #[test]
    fn generate_stored_key_matches_crypto_p_h_103() {
        let call = format!(
            "({KEY}(ia{{sv}}a{{sv}})(ayay(i)(i)(i)(i)xiiia{{sv}})(ssss(i)sa{{is}}(i)(i))a{{sv}}s)"
        );
        assert_eq!(
            signature::<GenerateStoredKeyCall>(),
            call,
            "{CRYPTO_ADAPTOR}:103"
        );
        assert_eq!(
            signature::<GenerateStoredKeyReply>(),
            format!("((iiis){KEY})"),
            "{CRYPTO_ADAPTOR}:110"
        );
    }

    #[test]
    fn encrypt_matches_crypto_p_h_211() {
        assert_eq!(
            signature::<EncryptCall>(),
            format!("(ayay{KEY}(i)(i)aya{{sv}}s)"),
            "{CRYPTO_ADAPTOR}:211"
        );
        assert_eq!(
            signature::<EncryptReply>(),
            "((iiis)ayay)",
            "{CRYPTO_ADAPTOR}:220"
        );
    }

    #[test]
    fn decrypt_matches_crypto_p_h_228() {
        assert_eq!(
            signature::<DecryptCall>(),
            format!("(ayay{KEY}(i)(i)ayaya{{sv}}s)"),
            "{CRYPTO_ADAPTOR}:228"
        );
        assert_eq!(
            signature::<DecryptReply>(),
            "((iiis)ay(i))",
            "{CRYPTO_ADAPTOR}:238"
        );
    }

    #[test]
    fn plugin_info_matches_the_phone_reply() {
        assert_eq!(
            signature::<SecretsPluginsReply>(),
            "((iis)a(ssii)a(ssii)a(ssii)a(ssii))",
            "dbus-send getPluginInfo on Sailfish OS 5.2.0.22"
        );
        assert_eq!(
            signature::<CryptoPluginsReply>(),
            "((iiis)a(ssii)a(ssii))",
            "dbus-send getPluginInfo on Sailfish OS 5.2.0.22"
        );
    }
}
