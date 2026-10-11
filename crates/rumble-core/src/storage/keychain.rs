use super::SecretStore;
use crate::{Error, Result};

/// OS の資格情報ストア (macOS キーチェーン / Windows 資格情報マネージャー /
/// Linux Secret Service) にパスワードを預ける。
pub struct KeychainSecretStore {
    service: String,
}

impl KeychainSecretStore {
    pub fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }

    fn entry(&self, account_id: &str) -> Result<keyring::Entry> {
        keyring::Entry::new(&self.service, account_id).map_err(secret_error)
    }
}

impl SecretStore for KeychainSecretStore {
    fn get(&self, account_id: &str) -> Result<String> {
        self.entry(account_id)?.get_password().map_err(secret_error)
    }

    fn set(&self, account_id: &str, password: &str) -> Result<()> {
        self.entry(account_id)?
            .set_password(password)
            .map_err(secret_error)
    }

    fn delete(&self, account_id: &str) -> Result<()> {
        match self.entry(account_id)?.delete_credential() {
            // すでに無いなら目的は達している
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(secret_error(e)),
        }
    }
}

fn secret_error(e: keyring::Error) -> Error {
    Error::Secret(e.to_string())
}
