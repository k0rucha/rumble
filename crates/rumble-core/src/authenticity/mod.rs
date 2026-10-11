//! 送信元の検証 (SPF / DKIM / DMARC / BIMI)。
//!
//! 「差出人欄のドメインが本当にこのメールを出したか」を、2 つの材料から判断する。
//!
//! 1. **自前の検算** ([`signature`]): DKIM 署名を、DNS から取った公開鍵で rumble 自身が
//!    確かめる。ヘッダーを偽造されても騙されない、いちばん強い根拠。
//! 2. **受信サーバーの申告** ([`reported`]): サーバーが受信時に行った検査の記録
//!    (Authentication-Results ヘッダー) を読む。SPF は接続元 IP が必要で受信サーバーに
//!    しか検査できないため、こちらも必要になる。ただし rumble からは検算できない。
//!
//! 2 つを 1 つの判定にまとめる規則は [`verdict`] に、ブランドロゴの取得は [`bimi`] にある。
//!
//! このモジュールは開いたメールについて DNS を引き、ロゴがあれば HTTPS で取得する。
//! 結果はメモリに持つだけで、ディスクには書かない。

mod bimi;
mod dns_cache;
mod reported;
mod signature;
mod verdict;

use std::sync::OnceLock;
use std::time::Duration;

use mail_auth::MessageAuthenticator;

use crate::domain::{DmarcPolicy, SenderAuth, TrustLevel};

/// DNS の応答を待つ上限。超えたら自前の検算はあきらめ、サーバーの申告だけで判定する。
const DNS_TIMEOUT: Duration = Duration::from_secs(10);

pub struct SenderVerifier {
    /// 初回の検証時に作る (作成には tokio ランタイムが要るため)
    dns: OnceLock<Option<MessageAuthenticator>>,
    dns_cache: dns_cache::DnsCache,
    logos: bimi::LogoFetcher,
}

impl SenderVerifier {
    pub fn new() -> Self {
        Self {
            dns: OnceLock::new(),
            dns_cache: dns_cache::DnsCache::default(),
            logos: bimi::LogoFetcher::new(),
        }
    }

    /// メール 1 通 (BODY[] の生データ) の送信元を検証する。
    /// 失敗はエラーにせず、結果の中に「確かめられなかった」として表す。
    pub async fn verify(&self, raw: &[u8]) -> SenderAuth {
        let reported = reported::from_message(raw);
        let checked = match self.dns() {
            Some(dns) => {
                let check = signature::check(dns, &self.dns_cache, raw);
                tokio::time::timeout(DNS_TIMEOUT, check).await.ok()
            }
            None => None,
        };
        let lookup_failed = checked.as_ref().is_none_or(|c| c.lookup_failed);
        let checked = checked.unwrap_or_else(|| signature::without_dns(raw));

        let level = verdict::judge(&checked.signatures, reported.as_ref());
        let logo = match (&checked.from_domain, self.dns()) {
            (Some(domain), Some(dns)) if deserves_logo(level, checked.dmarc_policy) => {
                self.logos.logo_for(dns, domain).await
            }
            _ => None,
        };

        SenderAuth {
            level,
            from_domain: checked.from_domain,
            signatures: checked.signatures,
            dmarc_policy: checked.dmarc_policy,
            reported,
            logo,
            lookup_failed,
        }
    }

    fn dns(&self) -> Option<&MessageAuthenticator> {
        self.dns.get_or_init(resolver).as_ref()
    }
}

/// OS に設定された DNS サーバーを使う。
#[cfg(not(target_os = "android"))]
fn resolver() -> Option<MessageAuthenticator> {
    MessageAuthenticator::new_system_conf()
        .or_else(|_| MessageAuthenticator::new_cloudflare())
        .ok()
}

/// Android では OS の DNS 設定を読むのに追加の権限 (ネットワーク状態の参照) が要るので、
/// 権限を増やさずに済むよう公開 DNS を直接使う。
#[cfg(target_os = "android")]
fn resolver() -> Option<MessageAuthenticator> {
    MessageAuthenticator::new_cloudflare().ok()
}

impl Default for SenderVerifier {
    fn default() -> Self {
        Self::new()
    }
}

/// ロゴを出してよいのは、送信元を確認でき、かつドメインが「失敗したメールは
/// 隔離・拒否せよ」と宣言している場合だけ (BIMI の要件)。
/// 誰でも名乗れる状態のドメインのロゴを出すと、なりすましにお墨付きを与えてしまう。
fn deserves_logo(level: TrustLevel, policy: Option<DmarcPolicy>) -> bool {
    matches!(level, TrustLevel::Verified | TrustLevel::Reported)
        && matches!(policy, Some(DmarcPolicy::Quarantine | DmarcPolicy::Reject))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 実際の DNS と HTTPS を使う。`cargo test -p rumble-core authenticity -- --ignored`
    #[tokio::test]
    #[ignore = "インターネット接続が必要"]
    async fn looks_up_policy_and_logo_on_the_real_internet() {
        let raw = b"Authentication-Results: mx.example.net; dmarc=pass header.from=cnn.com\r\nFrom: News <news@cnn.com>\r\nSubject: test\r\n\r\nbody\r\n";
        let auth = SenderVerifier::new().verify(raw).await;

        assert_eq!(auth.level, TrustLevel::Reported);
        assert_eq!(auth.dmarc_policy, Some(DmarcPolicy::Reject));
        assert!(!auth.lookup_failed);
        let logo = auth.logo.expect("cnn.com は BIMI のロゴを公開している");
        assert_eq!(logo.domain, "cnn.com");
        assert!(logo.data_url.starts_with("data:image/svg+xml;base64,"));
        assert!(logo.has_certificate);
    }

    #[test]
    fn logo_needs_a_confirmed_sender_and_an_enforcing_policy() {
        let enforcing = Some(DmarcPolicy::Reject);
        assert!(deserves_logo(TrustLevel::Verified, enforcing));
        assert!(deserves_logo(
            TrustLevel::Reported,
            Some(DmarcPolicy::Quarantine)
        ));
        assert!(!deserves_logo(
            TrustLevel::Verified,
            Some(DmarcPolicy::None)
        ));
        assert!(!deserves_logo(TrustLevel::Verified, None));
        assert!(!deserves_logo(TrustLevel::Partial, enforcing));
        assert!(!deserves_logo(TrustLevel::Failed, enforcing));
    }
}
