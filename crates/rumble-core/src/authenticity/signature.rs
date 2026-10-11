//! DKIM 署名の検算と、DMARC の方針の確認。
//!
//! 署名したドメインの公開鍵を DNS から取り、メールが署名後に書き換えられていないかを
//! rumble 自身が確かめる。暗号と DNS レコードの解釈は mail-auth に任せる。

use mail_auth::dkim::DkimError;
use mail_auth::dmarc::verify::DmarcParameters;
use mail_auth::dmarc::Policy;
use mail_auth::{
    AuthenticatedMessage, DkimOutput, DkimResult, DmarcOutput, DmarcResult, DnsError,
    Error as AuthError, MessageAuthenticator, Parameters, SpfOutput,
};

use super::dns_cache::DnsCache;

use crate::domain::{CheckStatus, DmarcPolicy, SignatureCheck};

pub struct Checked {
    pub from_domain: Option<String>,
    pub signatures: Vec<SignatureCheck>,
    pub dmarc_policy: Option<DmarcPolicy>,
    /// DNS の応答が得られず、確かめきれなかったものがある
    pub lookup_failed: bool,
}

pub async fn check(dns: &MessageAuthenticator, cache: &DnsCache, raw: &[u8]) -> Checked {
    let Some(message) = AuthenticatedMessage::parse(raw) else {
        return Checked::empty();
    };
    let from_domain = from_domain(&message);
    let outputs = dns
        .verify_dkim(Parameters::new(&message).with_txt_cache(cache))
        .await;
    let dmarc = dmarc_for(dns, cache, &message, &outputs).await;
    let dmarc_passes = dmarc.dkim_result() == &DmarcResult::Pass;

    let mut signatures = Vec::with_capacity(outputs.len());
    for output in &outputs {
        let Some(signature) = output.signature() else {
            continue; // 書式が壊れていて、誰の署名かも読めないもの
        };
        let (status, detail) = explain(output.result());
        let aligned = match &from_domain {
            Some(from) if covers(&signature.d, from) => true,
            // 兄弟ドメインなど、名前だけでは同じ組織か決められない場合は DMARC の規則で確かめる。
            // 合格した署名にしか意味が無いので、それ以外では問い合わせない
            Some(_) if status == CheckStatus::Pass && dmarc_passes => {
                let alone = std::slice::from_ref(output);
                dmarc_for(dns, cache, &message, alone).await.dkim_result() == &DmarcResult::Pass
            }
            _ => false,
        };
        signatures.push(SignatureCheck {
            domain: signature.d.to_ascii_lowercase(),
            selector: signature.s.clone(),
            status,
            aligned,
            detail,
        });
    }

    Checked {
        from_domain,
        lookup_failed: outputs
            .iter()
            .any(|output| matches!(output.result(), DkimResult::TempError(_)))
            || matches!(dmarc.dkim_result(), DmarcResult::TempError(_)),
        dmarc_policy: dmarc.dmarc_record().and_then(|_| match dmarc.policy() {
            Policy::None => Some(DmarcPolicy::None),
            Policy::Quarantine => Some(DmarcPolicy::Quarantine),
            Policy::Reject => Some(DmarcPolicy::Reject),
            Policy::Unspecified => None,
        }),
        signatures,
    }
}

/// DNS を使えないときの結果。署名があることだけを伝え、合否は「不明」にする。
pub fn without_dns(raw: &[u8]) -> Checked {
    let Some(message) = AuthenticatedMessage::parse(raw) else {
        return Checked::empty();
    };
    let from_domain = from_domain(&message);
    let signatures = message
        .dkim_headers
        .iter()
        .map(|header| SignatureCheck {
            domain: header.header.d.to_ascii_lowercase(),
            selector: header.header.s.clone(),
            status: CheckStatus::Inconclusive,
            aligned: from_domain
                .as_deref()
                .is_some_and(|from| covers(&header.header.d, from)),
            detail: Some(DNS_UNAVAILABLE.into()),
        })
        .collect();
    Checked {
        from_domain,
        signatures,
        dmarc_policy: None,
        lookup_failed: true,
    }
}

impl Checked {
    fn empty() -> Self {
        Checked {
            from_domain: None,
            signatures: Vec::new(),
            dmarc_policy: None,
            lookup_failed: false,
        }
    }
}

const DNS_UNAVAILABLE: &str = "DNS に問い合わせられませんでした";

/// 差出人欄 (From) のドメイン。複数のドメインが並んでいる場合は判定の対象にしない。
fn from_domain(message: &AuthenticatedMessage<'_>) -> Option<String> {
    let mut domains = message
        .from
        .iter()
        .filter_map(|address| address.rsplit_once('@'))
        .map(|(_, domain)| domain.to_ascii_lowercase());
    let first = domains.next()?;
    domains.all(|other| other == first).then_some(first)
}

/// 署名ドメインが差出人ドメインそのもの、またはその親ドメインか。
/// 親ドメインの持ち主はサブドメインも支配しているので、その署名は本人のものと言える。
fn covers(signing_domain: &str, from_domain: &str) -> bool {
    let signing = signing_domain.to_ascii_lowercase();
    from_domain == signing
        || from_domain
            .strip_suffix(&signing)
            .is_some_and(|rest| rest.ends_with('.'))
}

/// 与えた署名だけを材料に、差出人ドメインの DMARC を評価する。
/// SPF は受信サーバーにしか検査できないので「結果なし」として渡す。
async fn dmarc_for(
    dns: &MessageAuthenticator,
    cache: &DnsCache,
    message: &AuthenticatedMessage<'_>,
    signatures: &[DkimOutput<'_>],
) -> DmarcOutput {
    let no_spf = SpfOutput::new(String::new());
    let parameters = DmarcParameters::new(message, signatures, "", &no_spf);
    dns.verify_dmarc(Parameters::new(parameters).with_txt_cache(cache))
        .await
}

/// 検算の結果を、画面に出す合否と理由にする。
fn explain(result: &DkimResult) -> (CheckStatus, Option<String>) {
    let (status, detail) = match result {
        DkimResult::Pass => return (CheckStatus::Pass, None),
        DkimResult::None => return (CheckStatus::None, None),
        DkimResult::Neutral(AuthError::Dkim(DkimError::FailedBodyHashMatch)) => (
            CheckStatus::Fail,
            "署名のあとで本文が書き換えられています".to_string(),
        ),
        DkimResult::Fail(AuthError::Crypto(_)) => (
            CheckStatus::Fail,
            "署名が一致しません (ヘッダーが書き換えられたか、偽造された署名です)".to_string(),
        ),
        DkimResult::Fail(error) => (CheckStatus::Fail, error.to_string()),
        DkimResult::Neutral(AuthError::Dkim(DkimError::SignatureExpired)) => (
            CheckStatus::Inconclusive,
            "署名の有効期限が切れています".to_string(),
        ),
        DkimResult::TempError(_) => (CheckStatus::Inconclusive, DNS_UNAVAILABLE.to_string()),
        DkimResult::PermError(AuthError::Dns(DnsError::RecordNotFound(_))) => (
            CheckStatus::Inconclusive,
            "公開鍵が DNS にありません (古いメールでは鍵が更新済みのことがあります)".to_string(),
        ),
        DkimResult::PermError(AuthError::Dkim(DkimError::RevokedPublicKey)) => (
            CheckStatus::Inconclusive,
            "公開鍵が失効しています".to_string(),
        ),
        DkimResult::Neutral(error) | DkimResult::PermError(error) => {
            (CheckStatus::Inconclusive, error.to_string())
        }
    };
    (status, Some(detail))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use mail_auth::common::crypto::Ed25519Key;
    use mail_auth::common::headers::HeaderWriter;
    use mail_auth::common::parse::TxtRecordParser;
    use mail_auth::common::verify::DomainKey;
    use mail_auth::dkim::DkimSigner;
    use mail_auth::dmarc::Dmarc;
    use mail_auth::{ResolverCache, Txt};

    use super::*;

    // mail-auth の例にある試験用の鍵
    const PRIVATE_KEY: &str = "nWGxne/9WmC6hEr0kuwsxERJxWl7MmkZcDusAxyuf2A=";
    const PUBLIC_KEY: &str = "11qYAYKxCrfVS/7TyWQHOg7hcvPapiMlrwIaaPcHURo=";
    const MESSAGE: &str =
        "From: Bill <bill@news.signed.example>\r\nTo: jdoe@example.org\r\nSubject: TPS Report\r\n\r\nI need those reports.\r\n";

    /// `signing_domain` の鍵で署名したメール。
    fn signed_by(signing_domain: &str) -> String {
        let decode = |text: &str| {
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, text).unwrap()
        };
        let key = Ed25519Key::from_seed_and_public_key(&decode(PRIVATE_KEY), &decode(PUBLIC_KEY))
            .unwrap();
        let signature = DkimSigner::from_key(key)
            .domain(signing_domain)
            .selector("s1")
            .headers(["From", "To", "Subject"])
            .sign(MESSAGE.as_bytes())
            .unwrap();
        format!("{}{MESSAGE}", signature.to_header())
    }

    /// DNS の代わりに、試験用の公開鍵と DMARC レコードを答えるキャッシュ。
    /// 検証に必要な名前をすべて入れておくので、テストはネットワークに出ない。
    fn dns_answering_for(signing_domain: &str) -> DnsCache {
        let cache = DnsCache::default();
        let later = Instant::now() + Duration::from_secs(60);
        let key = DomainKey::parse(format!("v=DKIM1; k=ed25519; p={PUBLIC_KEY}").as_bytes());
        cache.insert(
            format!("s1._domainkey.{signing_domain}.").into(),
            Txt::DomainKey(Arc::new(key.unwrap())),
            later,
        );
        let dmarc = Dmarc::parse(b"v=DMARC1; p=reject").unwrap();
        for name in ["_dmarc.news.signed.example.", "_dmarc.signed.example."] {
            cache.insert(name.into(), Txt::Dmarc(Arc::new(dmarc.clone())), later);
        }
        cache
    }

    #[tokio::test]
    async fn valid_signature_of_the_from_domain_is_aligned_pass() {
        let dns = MessageAuthenticator::new_cloudflare().unwrap();
        let cache = dns_answering_for("signed.example");
        let checked = check(&dns, &cache, signed_by("signed.example").as_bytes()).await;

        assert_eq!(checked.from_domain.as_deref(), Some("news.signed.example"));
        assert_eq!(checked.signatures.len(), 1);
        assert_eq!(checked.signatures[0].status, CheckStatus::Pass);
        assert!(checked.signatures[0].aligned);
        assert_eq!(checked.dmarc_policy, Some(DmarcPolicy::Reject));
    }

    #[tokio::test]
    async fn tampered_body_fails() {
        let dns = MessageAuthenticator::new_cloudflare().unwrap();
        let cache = dns_answering_for("signed.example");
        let tampered = signed_by("signed.example").replace("those reports", "your password");
        let checked = check(&dns, &cache, tampered.as_bytes()).await;

        assert_eq!(checked.signatures[0].status, CheckStatus::Fail);
    }

    #[tokio::test]
    async fn valid_signature_of_another_domain_is_not_aligned() {
        let dns = MessageAuthenticator::new_cloudflare().unwrap();
        let cache = dns_answering_for("mailer.example");
        let checked = check(&dns, &cache, signed_by("mailer.example").as_bytes()).await;

        assert_eq!(checked.signatures[0].status, CheckStatus::Pass);
        assert!(!checked.signatures[0].aligned);
    }

    #[test]
    fn parent_domain_signature_covers_the_sender() {
        assert!(covers("example.com", "example.com"));
        assert!(covers("Example.com", "mail.example.com"));
        assert!(!covers("mail.example.com", "example.com"));
        assert!(!covers("example.com", "badexample.com"));
        assert!(!covers("sendgrid.net", "example.com"));
    }

    #[test]
    fn body_change_is_a_failure_but_expiry_is_not() {
        let changed = DkimResult::Neutral(AuthError::Dkim(DkimError::FailedBodyHashMatch));
        let expired = DkimResult::Neutral(AuthError::Dkim(DkimError::SignatureExpired));
        assert_eq!(explain(&changed).0, CheckStatus::Fail);
        assert_eq!(explain(&expired).0, CheckStatus::Inconclusive);
        assert_eq!(explain(&DkimResult::Pass), (CheckStatus::Pass, None));
    }

    #[test]
    fn without_dns_lists_signatures_as_inconclusive() {
        let raw = b"DKIM-Signature: v=1; a=rsa-sha256; c=relaxed/relaxed; d=example.com; s=s1;\r\n h=from:subject; bh=2jUSOH9NhtVGCQWNr9BrIAPreKQjO6Sn7XIkfJVOzv8=; b=dGVzdA==\r\nFrom: Alice <alice@news.example.com>\r\nSubject: hi\r\n\r\nbody\r\n";
        let checked = without_dns(raw);
        assert_eq!(checked.from_domain.as_deref(), Some("news.example.com"));
        assert_eq!(checked.signatures.len(), 1);
        assert_eq!(checked.signatures[0].status, CheckStatus::Inconclusive);
        assert!(checked.signatures[0].aligned);
        assert!(checked.lookup_failed);
    }
}
