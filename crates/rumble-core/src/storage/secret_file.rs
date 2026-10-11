use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use super::SecretStore;
use crate::{Error, Result};

/// パスワードを 1 つの JSON ファイルに保存する。
///
/// OS の資格情報ストアを使えない環境 (Android) 向け。**暗号化はしない**ので、
/// 他のアプリから読めない場所 (Android のアプリ専用領域) に置くこと。
/// 端末の root 権限を持つ相手からは守れない点が、キーチェーンより弱い。
pub struct FileSecretStore {
    path: PathBuf,
    /// 読んでから書き戻すまでを 1 つずつ行うための鍵
    lock: Mutex<()>,
}

impl FileSecretStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            lock: Mutex::new(()),
        }
    }

    fn read(&self) -> Result<BTreeMap<String, String>> {
        match fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(secret_error),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(e) => Err(secret_error(e)),
        }
    }

    fn write(&self, secrets: &BTreeMap<String, String>) -> Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir).map_err(secret_error)?;
        }
        let json = serde_json::to_vec(secrets).map_err(secret_error)?;
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, json).map_err(secret_error)?;
        restrict_to_owner(&tmp)?;
        fs::rename(&tmp, &self.path).map_err(secret_error)
    }
}

impl SecretStore for FileSecretStore {
    fn get(&self, account_id: &str) -> Result<String> {
        let _guard = self.lock.lock().unwrap();
        self.read()?
            .remove(account_id)
            .ok_or_else(|| Error::Secret("パスワードが保存されていません".into()))
    }

    fn set(&self, account_id: &str, password: &str) -> Result<()> {
        let _guard = self.lock.lock().unwrap();
        let mut secrets = self.read()?;
        secrets.insert(account_id.to_string(), password.to_string());
        self.write(&secrets)
    }

    fn delete(&self, account_id: &str) -> Result<()> {
        let _guard = self.lock.lock().unwrap();
        let mut secrets = self.read()?;
        secrets.remove(account_id);
        self.write(&secrets)
    }
}

/// 所有者以外が読めないようにする。
#[cfg(unix)]
fn restrict_to_owner(path: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(secret_error)
}

#[cfg(not(unix))]
fn restrict_to_owner(_path: &std::path::Path) -> Result<()> {
    Ok(())
}

fn secret_error(e: impl std::fmt::Display) -> Error {
    Error::Secret(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_and_deletes_passwords_per_account() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileSecretStore::new(dir.path().join("private/secrets.json"));
        assert!(store.get("a").is_err());

        store.set("a", "pw-a").unwrap();
        store.set("b", "pw-b").unwrap();
        assert_eq!(store.get("a").unwrap(), "pw-a");

        store.delete("a").unwrap();
        store.delete("a").unwrap(); // すでに無くてもエラーにしない
        assert!(store.get("a").is_err());
        assert_eq!(store.get("b").unwrap(), "pw-b");
    }

    #[cfg(unix)]
    #[test]
    fn file_is_readable_only_by_owner() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secrets.json");
        FileSecretStore::new(&path).set("a", "pw").unwrap();
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
