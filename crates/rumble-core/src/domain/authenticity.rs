use serde::Serialize;

/// 「差出人欄のドメインが本当にこのメールを出したか」の総合判定。
/// 画面ではこの値で色とアイコンを決める。判定の規則は `authenticity::verdict`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TrustLevel {
    /// rumble 自身が DKIM 署名を検算し、差出人ドメインのものだと確かめた
    Verified,
    /// rumble では確かめられなかったが、受信サーバーが DMARC 合格と記録している
    Reported,
    /// 署名や SPF は通っているが、差出人欄とは別のドメインのもの
    Partial,
    /// 検証に失敗した (なりすまし、または配送途中での改変)
    Failed,
    /// 判断材料が無い
    Unknown,
}

/// 個々の検査の結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CheckStatus {
    Pass,
    Fail,
    /// 合否を決められない (期限切れ、公開鍵が取れない、softfail など)
    Inconclusive,
    /// 検査の対象が無い
    None,
}

/// rumble が自分で検算した DKIM 署名 1 つ。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureCheck {
    /// 署名したドメイン (d=)
    pub domain: String,
    pub selector: String,
    pub status: CheckStatus,
    /// 差出人欄のドメインの署名とみなせるか
    pub aligned: bool,
    /// 合格以外のときの理由
    pub detail: Option<String>,
}

/// 差出人ドメインが DNS で宣言している DMARC の方針。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DmarcPolicy {
    /// 失敗しても何もしない (監視のみ)
    None,
    Quarantine,
    Reject,
}

/// 受信サーバーが Authentication-Results ヘッダーに書いた検査結果 1 行。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportedCheck {
    /// "spf" / "dkim" / "dmarc"
    pub method: String,
    pub status: CheckStatus,
    /// ヘッダーに書かれていた結果そのまま ("pass", "softfail" など)
    pub result: String,
    /// 検査対象のドメイン
    pub domain: Option<String>,
}

/// 受信サーバーの申告。rumble は内容を検算していない。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportedAuth {
    /// 検査したと名乗っているサーバー (authserv-id)
    pub verifier: String,
    pub checks: Vec<ReportedCheck>,
}

/// BIMI で差出人ドメインが公開しているブランドロゴ。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrandLogo {
    /// ロゴを公開しているドメイン
    pub domain: String,
    /// `<img>` にそのまま渡せる data: URL (SVG)
    pub data_url: String,
    /// 認証局の証明書 (VMC) が添えられているか。rumble は証明書を検証していない
    pub has_certificate: bool,
}

/// 開いたメール 1 通の、送信元の検証結果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SenderAuth {
    pub level: TrustLevel,
    /// 差出人欄 (From) のドメイン
    pub from_domain: Option<String>,
    pub signatures: Vec<SignatureCheck>,
    pub dmarc_policy: Option<DmarcPolicy>,
    pub reported: Option<ReportedAuth>,
    pub logo: Option<BrandLogo>,
    /// DNS に問い合わせられず、自前の検算ができなかった
    pub lookup_failed: bool,
}

/// メールの生データ (エンジニア向けの表示用)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageSource {
    /// ヘッダー部分。サーバーから受け取ったまま (折り返しもデコードもしない)
    pub headers: String,
    /// ヘッダーと本文を合わせた全体。大きすぎる場合は先頭だけ
    pub text: String,
    /// 全体のバイト数
    pub total_bytes: usize,
    pub truncated: bool,
}
