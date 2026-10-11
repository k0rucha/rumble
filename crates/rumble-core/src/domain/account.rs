use serde::{Deserialize, Serialize};

/// 接続の暗号化方式。平文接続は意図的にサポートしない。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Security {
    /// 最初から TLS (IMAP 993 / SMTP 465)
    Tls,
    /// 平文で接続してから STARTTLS で昇格 (IMAP 143 / SMTP 587)
    StartTls,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerEndpoint {
    pub host: String,
    pub port: u16,
    pub security: Security,
}

/// 保存されるアカウント設定。**パスワードは含まない** (OS のキーチェーンに置く)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    /// 差出人として表示する名前
    pub display_name: String,
    pub email: String,
    /// IMAP / SMTP 共通のログイン名
    pub username: String,
    pub imap: ServerEndpoint,
    pub smtp: ServerEndpoint,
}

/// アカウント追加フォームの入力。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewAccount {
    pub display_name: String,
    pub email: String,
    pub username: String,
    pub password: String,
    pub imap: ServerEndpoint,
    pub smtp: ServerEndpoint,
}
