//! IMAP まわり。
//!
//! - [`connection`] : 接続の確立と、切れたときのつなぎ直し
//! - [`ops`]        : 1 つのセッション上で実行する IMAP 操作 (状態を持たない関数群)
//! - [`mapping`]    : サーバーの応答 → [`crate::domain`] の型への変換
//! - [`watcher`]    : IDLE による受信トレイの監視

pub mod connection;
pub mod mapping;
pub mod ops;
pub mod watcher;

pub use connection::{ImapConnection, ImapSession};
