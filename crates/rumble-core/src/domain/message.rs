use serde::{Deserialize, Serialize};

use super::Category;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Address {
    pub name: Option<String>,
    pub email: String,
}

/// 一覧に出す 1 行ぶんの情報。本文は含まない (ヘッダーの一部だけを取得する)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageSummary {
    pub uid: u32,
    pub subject: String,
    pub from: Option<Address>,
    pub to: Vec<Address>,
    /// UNIX 秒。Date ヘッダーが壊れていればサーバーの受信日時で代用する
    pub date: Option<i64>,
    pub size: u32,
    pub seen: bool,
    pub flagged: bool,
    pub answered: bool,
    /// Content-Type からの推定 (multipart/mixed なら true)
    pub has_attachment: bool,
    /// 自動分類の結果。まだ分類していなければ None
    pub category: Option<Category>,
}

/// 一覧の 1 ページ。新しい順。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessagePage {
    /// フォルダ内 (検索時は一致した) メールの総数
    pub total: u32,
    pub messages: Vec<MessageSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentInfo {
    /// メール内での添付の通し番号。保存時にこの番号で指定する
    pub index: u32,
    pub filename: String,
    pub mime_type: String,
    pub size: usize,
}

/// 開いたメールの全体。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageBody {
    pub uid: u32,
    pub subject: String,
    pub from: Option<Address>,
    pub to: Vec<Address>,
    pub cc: Vec<Address>,
    pub reply_to: Vec<Address>,
    pub date: Option<i64>,
    pub message_id: Option<String>,
    /// 返信時に References ヘッダーへ引き継ぐ ID 列
    pub references: Vec<String>,
    /// サニタイズ済み HTML (HTML パートがある場合のみ)
    pub html: Option<String>,
    /// プレーンテキスト版 (HTML しか無いメールでは HTML から変換したもの)
    pub text: Option<String>,
    /// HTML が外部の画像などを参照しているか (既定では読み込まない)
    pub has_remote_content: bool,
    pub attachments: Vec<AttachmentInfo>,
}

/// rumble から切り替えられるフラグ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MessageFlag {
    Seen,
    Flagged,
    Answered,
}

impl MessageFlag {
    pub fn as_imap(self) -> &'static str {
        match self {
            MessageFlag::Seen => "\\Seen",
            MessageFlag::Flagged => "\\Flagged",
            MessageFlag::Answered => "\\Answered",
        }
    }
}
