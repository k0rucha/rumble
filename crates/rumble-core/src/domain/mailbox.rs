use serde::{Deserialize, Serialize};

/// フォルダの役割。サーバーの SPECIAL-USE 属性 (RFC 6154) から決める。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MailboxRole {
    // 並び順 = サイドバーでの表示順
    Inbox,
    Flagged,
    Drafts,
    Sent,
    Archive,
    All,
    Junk,
    Trash,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Mailbox {
    /// サーバー上の名前そのもの (modified UTF-7 のまま)。IMAP コマンドにはこれを使う。
    pub path: String,
    /// 表示用にデコードした末端の名前
    pub name: String,
    /// 階層の深さ (0 が最上位)
    pub depth: usize,
    pub role: MailboxRole,
    /// false のフォルダは入れ物でしかなく、メールを開けない
    pub selectable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MailboxStatus {
    pub total: u32,
    pub unseen: u32,
}
