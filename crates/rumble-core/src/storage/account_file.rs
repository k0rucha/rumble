use std::fs;
use std::path::PathBuf;

use super::AccountStore;
use crate::domain::Account;
use crate::{Error, Result};

/// アカウント設定を 1 つの JSON ファイルに保存する。
pub struct JsonAccountStore {
    path: PathBuf,
}

impl JsonAccountStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl AccountStore for JsonAccountStore {
    fn load(&self) -> Result<Vec<Account>> {
        match fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(config_error),
            // 初回起動時はファイルが無いのが普通
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(config_error(e)),
        }
    }

    fn save(&self, accounts: &[Account]) -> Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir).map_err(config_error)?;
        }
        let json = serde_json::to_vec_pretty(accounts).map_err(config_error)?;
        // 書き込み途中で落ちても設定を壊さないよう、別名で書いてから置き換える
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, json).map_err(config_error)?;
        fs::rename(&tmp, &self.path).map_err(config_error)
    }
}

fn config_error(e: impl std::fmt::Display) -> Error {
    Error::Config(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Security, ServerEndpoint};

    fn sample() -> Account {
        let endpoint = |port| ServerEndpoint {
            host: "mail.example.com".into(),
            port,
            security: Security::Tls,
        };
        Account {
            id: "a1".into(),
            display_name: "山田".into(),
            email: "yamada@example.com".into(),
            username: "yamada".into(),
            imap: endpoint(993),
            smtp: endpoint(465),
        }
    }

    #[test]
    fn missing_file_means_no_accounts() {
        let dir = tempfile::tempdir().unwrap();
        let store = JsonAccountStore::new(dir.path().join("accounts.json"));
        assert!(store.load().unwrap().is_empty());
    }

    #[test]
    fn saved_accounts_can_be_loaded_back() {
        let dir = tempfile::tempdir().unwrap();
        let store = JsonAccountStore::new(dir.path().join("nested/accounts.json"));
        store.save(&[sample()]).unwrap();
        assert_eq!(store.load().unwrap(), vec![sample()]);
    }

    #[test]
    fn password_is_never_written_to_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("accounts.json");
        JsonAccountStore::new(&path).save(&[sample()]).unwrap();
        let text = fs::read_to_string(path).unwrap();
        assert!(!text.to_lowercase().contains("password"));
    }
}
