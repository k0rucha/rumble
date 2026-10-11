//! 受信メール (RFC 5322 / MIME) の解釈。
//!
//! ネットワークにも状態にも触れない純粋な関数だけを置く。
//! 文字コード (ISO-2022-JP など) や RFC 2047 のデコードは mail-parser に任せる。

use base64::Engine;
use mail_parser::{HeaderValue, Message, MessageParser, MimeHeaders, PartType};

use crate::domain::{Address, AttachmentInfo, MessageBody};
use crate::{Error, Result};

/// 本文に埋め込む inline 画像 (cid:) の合計サイズ上限。
const MAX_INLINE_IMAGE_BYTES: usize = 8 * 1024 * 1024;

/// 一覧表示用に、ヘッダー部分だけから取り出した情報。
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SummaryHeaders {
    pub subject: String,
    pub from: Option<Address>,
    pub to: Vec<Address>,
    pub date: Option<i64>,
    pub has_attachment: bool,
}

pub fn parse_summary_headers(header_bytes: &[u8]) -> SummaryHeaders {
    let Some(message) = MessageParser::default().parse(header_bytes) else {
        return SummaryHeaders::default();
    };
    SummaryHeaders {
        subject: message.subject().unwrap_or_default().to_string(),
        from: addresses(message.from()).into_iter().next(),
        to: addresses(message.to()),
        date: message.date().map(|d| d.to_timestamp()),
        // 本文を取らずに分かる範囲での推定。multipart/mixed はほぼ添付つき
        has_attachment: message.root_part().content_type().is_some_and(|ct| {
            ct.ctype().eq_ignore_ascii_case("multipart")
                && ct
                    .subtype()
                    .is_some_and(|s| s.eq_ignore_ascii_case("mixed"))
        }),
    }
}

/// メール全体 (BODY[]) を画面表示用の形にする。
pub fn parse_message(uid: u32, raw: &[u8]) -> Result<MessageBody> {
    let message = parse(raw)?;

    let html = message
        .html_part(0)
        .and_then(|part| match &part.body {
            PartType::Html(html) => Some(html.as_ref()),
            _ => None, // テキストのみのメール。HTML へは変換せず text として見せる
        })
        .map(|html| embed_inline_images(sanitize_html(html), &message));

    Ok(MessageBody {
        uid,
        subject: message.subject().unwrap_or_default().to_string(),
        from: addresses(message.from()).into_iter().next(),
        to: addresses(message.to()),
        cc: addresses(message.cc()),
        reply_to: addresses(message.reply_to()),
        date: message.date().map(|d| d.to_timestamp()),
        message_id: message.message_id().map(str::to_string),
        references: id_list(message.references()),
        has_remote_content: html.as_deref().is_some_and(references_remote_content),
        html,
        text: message.body_text(0).map(|t| t.into_owned()),
        attachments: message
            .attachments()
            .enumerate()
            .map(|(index, part)| AttachmentInfo {
                index: index as u32,
                filename: attachment_filename(part, index),
                mime_type: mime_type(part),
                size: part.contents().len(),
            })
            .collect(),
    })
}

/// 添付ファイル 1 つの (ファイル名, 中身) を取り出す。
pub fn extract_attachment(raw: &[u8], index: u32) -> Result<(String, Vec<u8>)> {
    let message = parse(raw)?;
    let part = message
        .attachment(index)
        .ok_or_else(|| Error::NotFound("添付ファイルが見つかりません".into()))?;
    Ok((
        attachment_filename(part, index as usize),
        part.contents().to_vec(),
    ))
}

/// HTML メールを無害化する。
///
/// スクリプトやフォームは落とし、レイアウト用の style / 属性は残す。
/// 表示側でも sandbox 付き iframe + CSP で二重に守っている (HtmlBody.svelte)。
pub fn sanitize_html(html: &str) -> String {
    ammonia::Builder::default()
        .add_tags(["style", "font", "center"])
        .rm_clean_content_tags(["style"])
        .add_generic_attributes([
            "style", "class", "align", "valign", "bgcolor", "color", "width", "height", "dir",
        ])
        .add_tag_attributes("table", ["border", "cellpadding", "cellspacing"])
        .add_tag_attributes("font", ["face", "size"])
        .add_url_schemes(["cid"])
        .clean(html)
        .to_string()
}

fn parse(raw: &[u8]) -> Result<Message<'_>> {
    MessageParser::default()
        .parse(raw)
        .ok_or_else(|| Error::InvalidInput("メールを解釈できませんでした".into()))
}

fn addresses(header: Option<&mail_parser::Address<'_>>) -> Vec<Address> {
    let Some(header) = header else {
        return Vec::new();
    };
    header
        .iter()
        .filter_map(|addr| {
            Some(Address {
                name: addr.name().map(str::to_string).filter(|n| !n.is_empty()),
                email: addr.address()?.to_string(),
            })
        })
        .collect()
}

fn id_list(value: &HeaderValue<'_>) -> Vec<String> {
    match value {
        HeaderValue::Text(id) => vec![id.to_string()],
        HeaderValue::TextList(ids) => ids.iter().map(|id| id.to_string()).collect(),
        _ => Vec::new(),
    }
}

fn attachment_filename(part: &mail_parser::MessagePart<'_>, index: usize) -> String {
    match part.attachment_name() {
        Some(name) if !name.trim().is_empty() => name.to_string(),
        _ => format!("attachment-{}", index + 1),
    }
}

fn mime_type(part: &mail_parser::MessagePart<'_>) -> String {
    match part.content_type() {
        Some(ct) => match ct.subtype() {
            Some(subtype) => format!("{}/{}", ct.ctype(), subtype).to_lowercase(),
            None => ct.ctype().to_lowercase(),
        },
        None => "application/octet-stream".into(),
    }
}

/// `<img src="cid:...">` を、メールに同梱された画像の data: URI に置き換える。
/// これは同梱データなので外部通信は発生しない。
fn embed_inline_images(mut html: String, message: &Message<'_>) -> String {
    let mut budget = MAX_INLINE_IMAGE_BYTES;
    for part in &message.parts {
        let Some(content_id) = part.content_id() else {
            continue;
        };
        let (PartType::Binary(data) | PartType::InlineBinary(data)) = &part.body else {
            continue;
        };
        let mime = mime_type(part);
        let reference = format!("cid:{}", content_id.trim_matches(['<', '>']));
        if !mime.starts_with("image/") || data.len() > budget || !html.contains(&reference) {
            continue;
        }
        budget -= data.len();
        let encoded = base64::engine::general_purpose::STANDARD.encode(data);
        html = html.replace(&reference, &format!("data:{mime};base64,{encoded}"));
    }
    html
}

/// 外部から何かを読み込もうとする HTML か (ざっくりした判定)。
/// 見逃しても表示側の CSP が遮断するので、ここは「読み込みますか?」を出すか決めるだけ。
fn references_remote_content(html: &str) -> bool {
    let lower = html.to_ascii_lowercase();
    [
        "src=\"http",
        "background=\"http",
        "url(http",
        "url('http",
        "url(\"http",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_japanese_headers() {
        // ISO-2022-JP を B エンコードした昔ながらの日本語メール
        let headers = b"From: =?ISO-2022-JP?B?GyRCOzNFRBsoQg==?= <yamada@example.com>\r\n\
            To: a@example.com, B <b@example.com>\r\n\
            Subject: =?UTF-8?B?44GT44KT44Gr44Gh44Gv?=\r\n\
            Date: Sat, 10 Oct 2026 12:00:00 +0900\r\n\
            Content-Type: multipart/mixed; boundary=x\r\n\r\n";
        let summary = parse_summary_headers(headers);

        assert_eq!(summary.subject, "こんにちは");
        let from = summary.from.unwrap();
        assert_eq!(from.name.as_deref(), Some("山田"));
        assert_eq!(from.email, "yamada@example.com");
        assert_eq!(summary.to.len(), 2);
        assert_eq!(summary.date, Some(1_791_601_200));
        assert!(summary.has_attachment);
    }

    #[test]
    fn broken_headers_do_not_fail() {
        let summary = parse_summary_headers(b"");
        assert_eq!(summary.subject, "");
        assert!(summary.from.is_none());
    }

    const MULTIPART: &[u8] = b"From: A <a@example.com>\r\n\
        To: b@example.com\r\n\
        Subject: Report\r\n\
        Message-ID: <m2@example.com>\r\n\
        References: <m0@example.com> <m1@example.com>\r\n\
        MIME-Version: 1.0\r\n\
        Content-Type: multipart/mixed; boundary=\"outer\"\r\n\r\n\
        --outer\r\n\
        Content-Type: multipart/alternative; boundary=\"alt\"\r\n\r\n\
        --alt\r\n\
        Content-Type: text/plain; charset=utf-8\r\n\r\n\
        plain version\r\n\
        --alt\r\n\
        Content-Type: text/html; charset=utf-8\r\n\r\n\
        <p onclick=\"evil()\">html <b>version</b></p><script>evil()</script>\
        <img src=\"https://tracker.example.com/p.gif\">\r\n\
        --alt--\r\n\
        --outer\r\n\
        Content-Type: text/csv; name=\"data.csv\"\r\n\
        Content-Disposition: attachment; filename=\"data.csv\"\r\n\
        Content-Transfer-Encoding: base64\r\n\r\n\
        YSxiCjEsMgo=\r\n\
        --outer--\r\n";

    #[test]
    fn parses_bodies_and_attachments() {
        let body = parse_message(7, MULTIPART).unwrap();

        assert_eq!(body.uid, 7);
        assert_eq!(body.subject, "Report");
        assert_eq!(body.message_id.as_deref(), Some("m2@example.com"));
        assert_eq!(body.references, ["m0@example.com", "m1@example.com"]);
        assert_eq!(body.text.as_deref().map(str::trim), Some("plain version"));
        assert_eq!(body.attachments.len(), 1);
        assert_eq!(body.attachments[0].filename, "data.csv");
        assert_eq!(body.attachments[0].mime_type, "text/csv");
        assert_eq!(body.attachments[0].size, 8);
    }

    #[test]
    fn html_is_sanitized_and_remote_content_is_reported() {
        let body = parse_message(7, MULTIPART).unwrap();
        let html = body.html.unwrap();

        assert!(html.contains("<b>version</b>"));
        assert!(!html.contains("script"));
        assert!(!html.contains("onclick"));
        assert!(body.has_remote_content);
    }

    #[test]
    fn plain_text_mail_has_no_html() {
        let body = parse_message(1, b"Subject: hi\r\n\r\njust text\r\n").unwrap();
        assert!(body.html.is_none());
        assert_eq!(body.text.as_deref().map(str::trim), Some("just text"));
        assert!(!body.has_remote_content);
    }

    #[test]
    fn extracts_attachment_bytes() {
        let (name, bytes) = extract_attachment(MULTIPART, 0).unwrap();
        assert_eq!(name, "data.csv");
        assert_eq!(bytes, b"a,b\n1,2\n");
        assert!(extract_attachment(MULTIPART, 1).is_err());
    }

    #[test]
    fn sanitizer_keeps_layout_but_drops_active_content() {
        let clean = sanitize_html(
            "<style>p{color:red}</style><table cellpadding=\"4\"><tr><td style=\"padding:4px\">x</td></tr></table>\
             <a href=\"javascript:alert(1)\">a</a><iframe src=\"https://x\"></iframe><form><input></form>",
        );
        assert!(clean.contains("<style>p{color:red}</style>"));
        assert!(clean.contains("cellpadding=\"4\""));
        assert!(clean.contains("style=\"padding:4px\""));
        assert!(!clean.contains("javascript:"));
        assert!(!clean.contains("iframe"));
        assert!(!clean.contains("<form"));
        assert!(!clean.contains("<input"));
    }

    #[test]
    fn inline_images_are_embedded_as_data_uris() {
        let raw = b"Subject: logo\r\n\
            MIME-Version: 1.0\r\n\
            Content-Type: multipart/related; boundary=\"r\"\r\n\r\n\
            --r\r\n\
            Content-Type: text/html; charset=utf-8\r\n\r\n\
            <img src=\"cid:logo@x\">\r\n\
            --r\r\n\
            Content-Type: image/png\r\n\
            Content-ID: <logo@x>\r\n\
            Content-Transfer-Encoding: base64\r\n\r\n\
            iVBORw0KGgo=\r\n\
            --r--\r\n";
        let body = parse_message(1, raw).unwrap();
        let html = body.html.unwrap();

        assert!(
            html.contains("src=\"data:image/png;base64,iVBORw0KGgo=\""),
            "{html}"
        );
        assert!(!body.has_remote_content);
    }
}
