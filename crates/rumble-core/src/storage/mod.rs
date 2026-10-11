//! ディスクに触れる唯一のモジュール。
//!
//! rumble が永続化するのは次の 2 つだけ:
//!
//! 1. アカウント設定 (ホスト名など) — 小さな JSON ファイル
//! 2. パスワード — OS のキーチェーン (Android ではアプリ専用領域のファイル)
//!
//! メール本体・一覧・添付は **どれも保存しない**。
//! その境界をコード上でも明確にするため、保存先は trait で切り出している。

mod account_file;
#[cfg(not(target_os = "android"))]
mod keychain;
mod secret_file;

pub use account_file::JsonAccountStore;
#[cfg(not(target_os = "android"))]
pub use keychain::KeychainSecretStore;
pub use secret_file::FileSecretStore;

use crate::domain::Account;
use crate::Result;

/// アカウント設定の保存先。
pub trait AccountStore: Send + Sync {
    fn load(&self) -> Result<Vec<Account>>;
    fn save(&self, accounts: &[Account]) -> Result<()>;
}

/// パスワードの保存先。キーはアカウント ID。
pub trait SecretStore: Send + Sync {
    fn get(&self, account_id: &str) -> Result<String>;
    fn set(&self, account_id: &str, password: &str) -> Result<()>;
    fn delete(&self, account_id: &str) -> Result<()>;
}
