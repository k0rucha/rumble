//! rumble-core — rumble のメール処理ロジック。
//!
//! GUI (Tauri) には一切依存しない。レイヤーは下から順に:
//!
//! - [`domain`]  : 画面とやり取りする純粋なデータ型
//! - [`imap`] / [`smtp`] / [`mime`] : プロトコルごとの処理
//! - [`authenticity`] : 送信元の検証 (DKIM / DMARC / BIMI。DNS を引く)
//! - [`classifier`] : メールの自動分類 (外部の分類モデルを呼ぶ)
//! - [`storage`] : このクレートでディスクに触れる唯一の場所 (アカウント設定とパスワードのみ)
//! - [`cache`]   : メモリ上だけの一時キャッシュ
//! - [`service`] : 上記を束ねるユースケース層。外からはここだけを呼ぶ
//!
//! 設計上の大原則: **メールをディスクに保存しない**。
//! IMAP サーバーが唯一の正であり、rumble はそれを覗く窓に徹する。

pub mod authenticity;
pub mod cache;
pub mod classifier;
pub mod domain;
pub mod error;
pub mod imap;
pub mod mime;
pub mod service;
pub mod smtp;
pub mod storage;

pub use error::{Error, Result};
pub use service::MailService;
