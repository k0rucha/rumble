//! 個々の検査結果を、画面に出す 1 つの判定にまとめる。
//!
//! 規則 (上から順に当てはめる):
//!
//! 1. 差出人ドメインの DKIM 署名を rumble が検算して合格 → `Verified`
//! 2. 受信サーバーが DMARC 不合格と記録 → `Failed`
//! 3. 受信サーバーが DMARC 合格と記録 → `Reported`
//! 4. 差出人とは別のドメインについてだけ何かが合格 → `Partial`
//! 5. 合格が 1 つも無く、不合格がある → `Failed`
//! 6. それ以外 → `Unknown`
//!
//! 受信サーバーの申告より自前の検算を優先するのは、申告のヘッダーは送信者にも
//! 書けてしまうため (まともなサーバーは受信時に自分の名前のものを取り除くが、
//! rumble からはそれを確かめられない)。

use crate::domain::{CheckStatus, ReportedAuth, SignatureCheck, TrustLevel};

pub fn judge(signatures: &[SignatureCheck], reported: Option<&ReportedAuth>) -> TrustLevel {
    if signatures
        .iter()
        .any(|s| s.status == CheckStatus::Pass && s.aligned)
    {
        return TrustLevel::Verified;
    }

    let reported = |wanted: fn(&str) -> bool, status: CheckStatus| {
        reported.is_some_and(|r| {
            r.checks
                .iter()
                .any(|check| wanted(&check.method) && check.status == status)
        })
    };
    let dmarc = |method: &str| method == "dmarc";
    let others = |method: &str| method != "dmarc";

    if reported(dmarc, CheckStatus::Fail) {
        return TrustLevel::Failed;
    }
    if reported(dmarc, CheckStatus::Pass) {
        return TrustLevel::Reported;
    }

    let has = |status| signatures.iter().any(|s| s.status == status) || reported(others, status);
    if has(CheckStatus::Pass) {
        TrustLevel::Partial
    } else if has(CheckStatus::Fail) {
        TrustLevel::Failed
    } else {
        TrustLevel::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ReportedCheck;

    fn signature(status: CheckStatus, aligned: bool) -> SignatureCheck {
        SignatureCheck {
            domain: "example.com".into(),
            selector: "s1".into(),
            status,
            aligned,
            detail: None,
        }
    }

    fn report(checks: &[(&str, CheckStatus)]) -> ReportedAuth {
        ReportedAuth {
            verifier: "mx.example.net".into(),
            checks: checks
                .iter()
                .map(|&(method, status)| ReportedCheck {
                    method: method.into(),
                    status,
                    result: String::new(),
                    domain: None,
                })
                .collect(),
        }
    }

    #[test]
    fn own_check_of_the_from_domain_is_the_strongest_evidence() {
        let signatures = [signature(CheckStatus::Pass, true)];
        // サーバーが不合格と言っていても、自分で確かめた結果を採る
        let reported = report(&[("dmarc", CheckStatus::Fail)]);
        assert_eq!(judge(&signatures, Some(&reported)), TrustLevel::Verified);
    }

    #[test]
    fn server_report_decides_when_own_check_is_not_conclusive() {
        let expired = [signature(CheckStatus::Inconclusive, true)];
        let pass = report(&[("dmarc", CheckStatus::Pass)]);
        let fail = report(&[("spf", CheckStatus::Pass), ("dmarc", CheckStatus::Fail)]);
        assert_eq!(judge(&expired, Some(&pass)), TrustLevel::Reported);
        assert_eq!(judge(&expired, Some(&fail)), TrustLevel::Failed);
    }

    #[test]
    fn pass_for_another_domain_is_only_partial() {
        let signatures = [signature(CheckStatus::Pass, false)];
        assert_eq!(judge(&signatures, None), TrustLevel::Partial);
        // 差出人ドメインの署名が壊れていても、配送サービスの署名が通っていれば「一部」
        let relayed = [
            signature(CheckStatus::Fail, true),
            signature(CheckStatus::Pass, false),
        ];
        assert_eq!(judge(&relayed, None), TrustLevel::Partial);
        let spf_only = report(&[("spf", CheckStatus::Pass)]);
        assert_eq!(judge(&[], Some(&spf_only)), TrustLevel::Partial);
    }

    #[test]
    fn failure_without_any_pass_is_failed() {
        let signatures = [signature(CheckStatus::Fail, true)];
        assert_eq!(judge(&signatures, None), TrustLevel::Failed);
        let reported = report(&[("spf", CheckStatus::Fail), ("dkim", CheckStatus::None)]);
        assert_eq!(judge(&[], Some(&reported)), TrustLevel::Failed);
    }

    #[test]
    fn nothing_to_go_on_is_unknown() {
        assert_eq!(judge(&[], None), TrustLevel::Unknown);
        let inconclusive = report(&[("spf", CheckStatus::Inconclusive)]);
        assert_eq!(judge(&[], Some(&inconclusive)), TrustLevel::Unknown);
    }
}
