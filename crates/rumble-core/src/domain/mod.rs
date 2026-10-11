//! 画面 (フロントエンド) とやり取りするデータ型。
//!
//! ここには振る舞いをほとんど置かない。JSON では camelCase になる。

mod account;
mod mailbox;
mod message;
mod outgoing;

pub use account::{Account, NewAccount, Security, ServerEndpoint};
pub use mailbox::{Mailbox, MailboxRole, MailboxStatus};
pub use message::{Address, AttachmentInfo, MessageBody, MessageFlag, MessagePage, MessageSummary};
pub use outgoing::{MessageRef, OutgoingMessage, SendReport, SentCopy};

/// サーバー側の変化をフロントエンドへ知らせるイベント。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum MailEvent {
    /// IDLE で受信トレイの変化 (新着・既読化・削除など) を検知した。
    #[serde(rename_all = "camelCase")]
    MailboxChanged { account_id: String, mailbox: String },
}
