//! 受信サーバーの申告 (Authentication-Results ヘッダー, RFC 8601) を読む。
//!
//! 例:
//! ```text
//! Authentication-Results: mx.google.com;
//!        dkim=pass header.i=@example.com header.s=s1 header.b=AbCdEf;
//!        spf=pass (google.com: domain of a@example.com designates ...) smtp.mailfrom=a@example.com;
//!        dmarc=pass (p=REJECT sp=REJECT dis=NONE) header.from=example.com
//! ```

use mail_parser::MessageParser;

use crate::domain::{CheckStatus, ReportedAuth, ReportedCheck};

/// 判定に使う検査。これ以外 (arc, iprev など) は読み飛ばす。
const METHODS: [&str; 3] = ["spf", "dkim", "dmarc"];

/// メールのいちばん上にある Authentication-Results を読む。
///
/// ヘッダーは配送の途中で上へ上へと積まれるので、いちばん上が「最後に受け取った
/// サーバー = 自分のメールサーバー」のもの。それより下は送信側でも書けるので使わない。
pub fn from_message(raw: &[u8]) -> Option<ReportedAuth> {
    let message = MessageParser::default().parse_headers(raw)?;
    let header = message
        .headers()
        .iter()
        .find(|header| header.name().eq_ignore_ascii_case("Authentication-Results"))?;
    let value = raw.get(header.offset_start as usize..header.offset_end as usize)?;
    parse(&String::from_utf8_lossy(value))
}

/// ヘッダーの値 (名前とコロンを除いた部分) を解釈する。
pub fn parse(value: &str) -> Option<ReportedAuth> {
    let value = without_comments(value);
    let mut sections = value.split(';');
    // 先頭は検査したサーバーの名前 (後ろにバージョン番号が付くことがある)
    let verifier = sections.next()?.split_whitespace().next()?.to_string();

    let checks = sections
        .filter_map(|section| {
            let mut words = section.split_whitespace();
            let (method, result) = words.next()?.split_once('=')?;
            let method = method.to_ascii_lowercase();
            if !METHODS.contains(&method.as_str()) {
                return None;
            }
            let result = result.to_ascii_lowercase();
            let properties: Vec<(&str, &str)> =
                words.filter_map(|word| word.split_once('=')).collect();
            Some(ReportedCheck {
                status: status_of(&result),
                domain: checked_domain(&method, &properties),
                method,
                result,
            })
        })
        .collect();
    Some(ReportedAuth { verifier, checks })
}

fn status_of(result: &str) -> CheckStatus {
    match result {
        "pass" => CheckStatus::Pass,
        "fail" | "hardfail" | "policy" => CheckStatus::Fail,
        "none" => CheckStatus::None,
        // softfail / neutral / temperror / permerror など: 合否どちらとも言えない
        _ => CheckStatus::Inconclusive,
    }
}

/// その検査が「どのドメインについてのものか」を取り出す。
fn checked_domain(method: &str, properties: &[(&str, &str)]) -> Option<String> {
    let keys: &[&str] = match method {
        "spf" => &["smtp.mailfrom", "smtp.helo"],
        "dkim" => &["header.d", "header.i"],
        _ => &["header.from"],
    };
    let value = keys.iter().find_map(|key| {
        properties
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(key))
            .map(|(_, value)| *value)
    })?;
    // "user@example.com" や "@example.com" はドメイン部分だけにする
    let domain = value.rsplit('@').next()?.trim_matches('"');
    (!domain.is_empty()).then(|| domain.to_ascii_lowercase())
}

/// 括弧で囲まれたコメントを取り除く (入れ子にも対応)。
fn without_comments(value: &str) -> String {
    let mut depth = 0u32;
    let mut output = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '(' => depth += 1,
            ')' if depth > 0 => {
                depth -= 1;
                output.push(' ');
            }
            _ if depth == 0 => output.push(character),
            _ => {}
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_typical_gmail_header() {
        let reported = parse(
            "mx.google.com;\r\n       dkim=pass header.i=@example.com header.s=s1 header.b=AbCd;\r\n       spf=pass (google.com: domain of bounce@mail.example.com designates 192.0.2.1 as permitted sender) smtp.mailfrom=bounce@mail.example.com;\r\n       dmarc=pass (p=REJECT sp=REJECT dis=NONE) header.from=example.com",
        )
        .unwrap();
        assert_eq!(reported.verifier, "mx.google.com");
        let summary: Vec<_> = reported
            .checks
            .iter()
            .map(|c| (c.method.as_str(), c.status, c.domain.as_deref()))
            .collect();
        assert_eq!(
            summary,
            [
                ("dkim", CheckStatus::Pass, Some("example.com")),
                ("spf", CheckStatus::Pass, Some("mail.example.com")),
                ("dmarc", CheckStatus::Pass, Some("example.com")),
            ]
        );
    }

    #[test]
    fn maps_results_to_statuses() {
        let reported = parse(
            "mx.example.net 1; spf=softfail smtp.mailfrom=a@x.example; dkim=none; dmarc=fail header.from=x.example; arc=pass",
        )
        .unwrap();
        let statuses: Vec<_> = reported.checks.iter().map(|c| c.status).collect();
        assert_eq!(
            statuses,
            [
                CheckStatus::Inconclusive,
                CheckStatus::None,
                CheckStatus::Fail
            ]
        );
        assert_eq!(reported.checks[0].result, "softfail");
    }

    #[test]
    fn header_without_results_still_names_the_verifier() {
        let reported = parse("mx.example.net; none").unwrap();
        assert_eq!(reported.verifier, "mx.example.net");
        assert!(reported.checks.is_empty());
    }

    #[test]
    fn only_the_topmost_header_is_used() {
        let raw = b"Received: by mx.example.net\r\nAuthentication-Results: mx.example.net;\r\n dmarc=fail header.from=bank.example\r\nAuthentication-Results: mx.example.net; dmarc=pass header.from=bank.example\r\nFrom: a@bank.example\r\n\r\nbody";
        let reported = from_message(raw).unwrap();
        assert_eq!(reported.checks.len(), 1);
        assert_eq!(reported.checks[0].status, CheckStatus::Fail);
    }

    #[test]
    fn message_without_the_header_has_no_report() {
        assert_eq!(from_message(b"From: a@example.com\r\n\r\nbody"), None);
    }
}
