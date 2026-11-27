use keyring::credential::{
    Credential, CredentialApi, CredentialBuilder, CredentialBuilderApi, CredentialPersistence,
};
use keyring::{Error, Result};
use std::any::Any;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

type Store = Mutex<HashMap<(String, String), Vec<u8>>>;

fn store() -> &'static Store {
    static STORE: OnceLock<Store> = OnceLock::new();
    STORE.get_or_init(Default::default)
}

struct MapCredential {
    key: (String, String),
}

impl CredentialApi for MapCredential {
    fn set_secret(&self, secret: &[u8]) -> Result<()> {
        let mut store = store()
            .lock()
            .map_err(|err| Error::NoStorageAccess(err.to_string().into()))?;
        store.insert(self.key.clone(), secret.to_vec());
        Ok(())
    }

    fn get_secret(&self) -> Result<Vec<u8>> {
        let store = store()
            .lock()
            .map_err(|err| Error::NoStorageAccess(err.to_string().into()))?;
        store.get(&self.key).cloned().ok_or(Error::NoEntry)
    }

    fn delete_credential(&self) -> Result<()> {
        let mut store = store()
            .lock()
            .map_err(|err| Error::NoStorageAccess(err.to_string().into()))?;
        store.remove(&self.key).map(|_| ()).ok_or(Error::NoEntry)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct MapBuilder;

impl CredentialBuilderApi for MapBuilder {
    fn build(
        &self,
        _target: Option<&str>,
        service: &str,
        user: &str,
    ) -> Result<Box<Credential>> {
        Ok(Box::new(MapCredential {
            key: (service.to_owned(), user.to_owned()),
        }))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn persistence(&self) -> CredentialPersistence {
        CredentialPersistence::ProcessOnly
    }
}

pub fn install() {
    let builder: Box<CredentialBuilder> = Box::new(MapBuilder);
    keyring::set_default_credential_builder(builder);
}
