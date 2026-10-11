use serde::{Deserialize, Serialize};

/// サーバー上の 1 通を指す参照。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageRef {
    pub mailbox: String,
    pub uid: u32,
}

/// 作成画面から送られてくる送信内容。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutgoingMessage {
    /// `name <addr>` 形式または `addr` のみ
    pub to: Vec<String>,
    #[serde(default)]
    pub cc: Vec<String>,
    #[serde(default)]
    pub bcc: Vec<String>,
    pub subject: String,
    pub body: String,
    /// 添付するローカルファイルのパス
    #[serde(default)]
    pub attachments: Vec<String>,
    /// 返信元の Message-ID (In-Reply-To になる)
    #[serde(default)]
    pub in_reply_to: Option<String>,
    #[serde(default)]
    pub references: Vec<String>,
    /// 返信元メール。送信後に「返信済み」フラグを立てる
    #[serde(default)]
    pub replied_to: Option<MessageRef>,
}

/// 送信済みフォルダへの控えがどうなったか。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "detail")]
pub enum SentCopy {
    /// Gmail のようにサーバーが自動で保存する
    ServerManaged,
    /// rumble が IMAP APPEND で保存した
    Appended,
    /// 送信自体は成功したが控えを保存できなかった (理由つき)
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendReport {
    pub sent_copy: SentCopy,
}
