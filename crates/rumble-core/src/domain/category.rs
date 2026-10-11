use serde::{Deserialize, Serialize};

/// 自動分類の区分。
///
/// 分類結果は IMAP のキーワード (利用者定義フラグ) としてサーバー上のメールに付ける。
/// ローカルには保存しないので、他の端末の rumble からも同じ分類が見える。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Category {
    /// 人が自分宛てに書いたメール
    Personal,
    /// サービスからの自動通知
    Notification,
    /// 領収書・請求・注文や予約の確認
    Receipt,
    /// ニュースレター・宣伝
    Newsletter,
    /// どれにも当てはまらない、または判断がつかなかった。
    /// 「分類済み」の印でもあり、同じメールを何度も分類し直さないために保存する
    Other,
}

impl Category {
    pub const ALL: [Category; 5] = [
        Category::Personal,
        Category::Notification,
        Category::Receipt,
        Category::Newsletter,
        Category::Other,
    ];

    /// 分類モデルとのやり取りに使う名前。
    pub fn id(self) -> &'static str {
        match self {
            Category::Personal => "personal",
            Category::Notification => "notification",
            Category::Receipt => "receipt",
            Category::Newsletter => "newsletter",
            Category::Other => "other",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|category| category.id() == id)
    }

    /// サーバー上のメールに付ける IMAP キーワード。
    pub fn keyword(self) -> String {
        format!("{KEYWORD_PREFIX}{}", self.id())
    }

    /// IMAP キーワードから区分を読み取る (キーワードは大文字小文字を区別しない)。
    pub fn from_keyword(keyword: &str) -> Option<Self> {
        let keyword = keyword.to_ascii_lowercase();
        Self::from_id(keyword.strip_prefix(KEYWORD_PREFIX)?)
    }
}

const KEYWORD_PREFIX: &str = "$rumble_";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyword_round_trips_for_every_category() {
        for category in Category::ALL {
            assert_eq!(Category::from_keyword(&category.keyword()), Some(category));
        }
    }

    #[test]
    fn reads_keywords_case_insensitively_and_ignores_foreign_ones() {
        assert_eq!(
            Category::from_keyword("$Rumble_Newsletter"),
            Some(Category::Newsletter)
        );
        assert_eq!(Category::from_keyword("$Forwarded"), None);
        assert_eq!(Category::from_keyword("$rumble_unknown"), None);
    }
}
