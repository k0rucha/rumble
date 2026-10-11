//! BIMI: 差出人ドメインが DNS で公開しているブランドロゴを取得する。
//!
//! `default._bimi.<ドメイン>` の TXT レコード (`v=BIMI1; l=<SVG の URL>; a=<証明書の URL>`)
//! を引き、`l=` の SVG を HTTPS で取る。
//!
//! 注意: 証明書 (VMC) の検証はしていない。つまりロゴは「そのドメインの持ち主が
//! 自分で掲げているもの」であり、第三者が商標を確認したものとは限らない。
//! 画面でもそのように表示する (`has_certificate` は証明書が添えられているかだけを表す)。

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use base64::Engine;
use mail_auth::MessageAuthenticator;

use crate::domain::BrandLogo;

/// ロゴとして受け取る SVG の上限。BIMI の仕様は 32 KB 以下を求めているので十分な余裕。
const MAX_LOGO_BYTES: usize = 256 * 1024;
const FETCH_TIMEOUT: Duration = Duration::from_secs(8);
/// 親ドメインをいくつまで遡ってレコードを探すか (news.mail.example.com → example.com)。
const MAX_PARENT_LOOKUPS: usize = 3;

pub struct LogoFetcher {
    http: reqwest::Client,
    /// ドメインごとの取得結果 (無かった場合も覚える)。メモリのみで、終了すれば消える。
    /// 同じ差出人のメールを開くたびに取りに行かないためのもの
    known: Mutex<HashMap<String, Option<BrandLogo>>>,
}

impl LogoFetcher {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(FETCH_TIMEOUT)
                .redirect(reqwest::redirect::Policy::limited(3))
                .build()
                .unwrap_or_default(),
            known: Mutex::default(),
        }
    }

    pub async fn logo_for(
        &self,
        dns: &MessageAuthenticator,
        from_domain: &str,
    ) -> Option<BrandLogo> {
        if let Some(known) = self.known.lock().unwrap().get(from_domain) {
            return known.clone();
        }
        let logo = self.fetch(dns, from_domain).await;
        self.known
            .lock()
            .unwrap()
            .insert(from_domain.to_string(), logo.clone());
        logo
    }

    async fn fetch(&self, dns: &MessageAuthenticator, from_domain: &str) -> Option<BrandLogo> {
        // メールごとに別のセレクター (BIMI-Selector ヘッダー) は使わず、常に default を引く。
        // 送信者が宛先ごとに違う名前を仕込んで開封を検知する、という使い方をさせないため
        let (domain, record) = find_record(dns, from_domain).await?;
        let svg = self.download(record.logo_url.as_deref()?).await?;
        Some(BrandLogo {
            domain,
            data_url: format!(
                "data:image/svg+xml;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(svg)
            ),
            has_certificate: record.has_certificate,
        })
    }

    async fn download(&self, url: &str) -> Option<Vec<u8>> {
        if !url.starts_with("https://") {
            return None;
        }
        let mut response = self.http.get(url).send().await.ok()?;
        if !response.status().is_success() {
            return None;
        }
        let mut svg = Vec::new();
        while let Some(chunk) = response.chunk().await.ok()? {
            svg.extend_from_slice(&chunk);
            if svg.len() > MAX_LOGO_BYTES {
                return None;
            }
        }
        // 画像として <img> で描くので中のスクリプトは動かないが、SVG 以外は受け取らない
        looks_like_svg(&svg).then_some(svg)
    }
}

/// 差出人ドメインから親へ遡りながら、最初に見つかった BIMI レコードを返す。
async fn find_record(dns: &MessageAuthenticator, from_domain: &str) -> Option<(String, Record)> {
    let mut domain = from_domain;
    for _ in 0..MAX_PARENT_LOOKUPS {
        if let Ok(text) = dns.txt_raw_lookup(format!("default._bimi.{domain}.")).await {
            if let Some(record) = parse_record(&String::from_utf8_lossy(&text)) {
                return Some((domain.to_string(), record));
            }
        }
        // "example.com" より上 (com) は誰のものでもないので探さない
        let (_, parent) = domain.split_once('.')?;
        if !parent.contains('.') {
            return None;
        }
        domain = parent;
    }
    None
}

#[derive(Debug, PartialEq, Eq)]
struct Record {
    /// 空 (`l=;`) は「ロゴを出さない」という宣言
    logo_url: Option<String>,
    has_certificate: bool,
}

fn parse_record(text: &str) -> Option<Record> {
    let mut tags = text
        .split(';')
        .filter_map(|tag| tag.split_once('='))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim()));
    let (name, version) = tags.next()?;
    if name != "v" || !version.eq_ignore_ascii_case("BIMI1") {
        return None;
    }
    let mut record = Record {
        logo_url: None,
        has_certificate: false,
    };
    for (name, value) in tags {
        match name.as_str() {
            "l" if !value.is_empty() => record.logo_url = Some(value.to_string()),
            "a" => record.has_certificate = !value.is_empty(),
            _ => {}
        }
    }
    Some(record)
}

fn looks_like_svg(bytes: &[u8]) -> bool {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(2048)]);
    head.contains("<svg")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_logo_and_certificate_locations() {
        let record =
            parse_record("v=BIMI1; l=https://example.com/logo.svg; a=https://example.com/vmc.pem")
                .unwrap();
        assert_eq!(
            record.logo_url.as_deref(),
            Some("https://example.com/logo.svg")
        );
        assert!(record.has_certificate);
    }

    #[test]
    fn empty_logo_means_the_domain_declines() {
        let record = parse_record("v=BIMI1; l=; a=;").unwrap();
        assert_eq!(
            record,
            Record {
                logo_url: None,
                has_certificate: false
            }
        );
    }

    #[test]
    fn other_txt_records_are_ignored() {
        assert_eq!(parse_record("v=spf1 include:example.com ~all"), None);
        assert_eq!(parse_record("l=https://example.com/logo.svg"), None);
    }

    #[test]
    fn only_svg_is_accepted() {
        assert!(looks_like_svg(
            b"<?xml version=\"1.0\"?><svg xmlns=\"http://www.w3.org/2000/svg\"/>"
        ));
        assert!(!looks_like_svg(b"<html><body>404</body></html>"));
        assert!(!looks_like_svg(b"\x89PNG\r\n"));
    }
}
