//! クレート共通のエラー型。
//!
//! メッセージはそのまま画面に表示されるので、利用者が読んで分かる日本語にしている。

use serde::{Serialize, Serializer};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("ログインに失敗しました。ユーザー名とパスワードを確認してください ({0})")]
    Auth(String),

    #[error("IMAP サーバーとのやり取りに失敗しました: {0}")]
    Imap(#[from] async_imap::error::Error),

    #[error("ネットワークエラー: {0}")]
    Io(#[from] std::io::Error),

    #[error("TLS 接続に失敗しました: {0}")]
    Tls(#[from] async_native_tls::Error),

    #[error("サーバーが応答しません (タイムアウト)")]
    Timeout,

    #[error("メールの送信に失敗しました: {0}")]
    Smtp(String),

    #[error("パスワードの保管庫にアクセスできません: {0}")]
    Secret(String),

    #[error("設定ファイルを読み書きできません: {0}")]
    Config(String),

    #[error("{0}")]
    NotFound(String),

    #[error("{0}")]
    InvalidInput(String),
}

impl Error {
    /// 接続そのものが壊れた可能性があるエラーか。
    /// true の場合、その IMAP セッションは破棄して次回つなぎ直す。
    pub fn is_connection_broken(&self) -> bool {
        use async_imap::error::Error as Imap;
        matches!(
            self,
            Error::Io(_)
                | Error::Tls(_)
                | Error::Timeout
                | Error::Imap(Imap::Io(_) | Imap::ConnectionLost | Imap::Parse(_))
        )
    }
}

/// フロントエンドへは表示用の文字列として渡す。
impl Serialize for Error {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
