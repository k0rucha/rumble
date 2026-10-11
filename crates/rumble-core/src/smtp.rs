//! SMTP によるメール送信。

use std::path::Path;
use std::time::Duration;

use lettre::message::header::ContentType;
use lettre::message::{Attachment, Mailbox, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use crate::domain::{Account, OutgoingMessage, Security};
use crate::{Error, Result};

const SMTP_TIMEOUT: Duration = Duration::from_secs(60);

/// 送信し、送ったメールそのもの (RFC 5322 形式) を返す。
/// 返り値は「送信済み」フォルダへ控えを置くのに使う。
pub async fn send(
    account: &Account,
    password: &str,
    outgoing: &OutgoingMessage,
) -> Result<Vec<u8>> {
    let message = build_message(account, outgoing).await?;
    let raw = message.formatted();
    transport(account, password)?
        .send(message)
        .await
        .map_err(smtp_error)?;
    Ok(raw)
}

/// 設定した SMTP サーバーへログインできるか確かめる (アカウント追加時)。
pub async fn verify(account: &Account, password: &str) -> Result<()> {
    match transport(account, password)?.test_connection().await {
        Ok(true) => Ok(()),
        Ok(false) => Err(Error::Smtp("SMTP サーバーに接続できません".into())),
        Err(e) => Err(smtp_error(e)),
    }
}

fn transport(account: &Account, password: &str) -> Result<AsyncSmtpTransport<Tokio1Executor>> {
    let host = account.smtp.host.as_str();
    let builder = match account.smtp.security {
        Security::Tls => AsyncSmtpTransport::<Tokio1Executor>::relay(host),
        Security::StartTls => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host),
    }
    .map_err(smtp_error)?;

    Ok(builder
        .port(account.smtp.port)
        .credentials(Credentials::new(
            account.username.clone(),
            password.to_string(),
        ))
        .timeout(Some(SMTP_TIMEOUT))
        .build())
}

async fn build_message(account: &Account, outgoing: &OutgoingMessage) -> Result<Message> {
    if outgoing.to.is_empty() && outgoing.cc.is_empty() && outgoing.bcc.is_empty() {
        return Err(Error::InvalidInput("宛先を入力してください".into()));
    }

    let from_address = account.email.parse().map_err(|_| {
        Error::InvalidInput(format!(
            "差出人アドレスが正しくありません: {}",
            account.email
        ))
    })?;
    let display_name = Some(account.display_name.trim().to_string()).filter(|n| !n.is_empty());

    let mut builder = Message::builder()
        .from(Mailbox::new(display_name, from_address))
        .subject(outgoing.subject.as_str())
        .message_id(None)
        .user_agent(concat!("rumble/", env!("CARGO_PKG_VERSION")).to_string());

    for recipient in &outgoing.to {
        builder = builder.to(parse_recipient(recipient)?);
    }
    for recipient in &outgoing.cc {
        builder = builder.cc(parse_recipient(recipient)?);
    }
    for recipient in &outgoing.bcc {
        builder = builder.bcc(parse_recipient(recipient)?);
    }

    // スレッドがつながるよう、返信元の ID を引き継ぐ
    if let Some(parent) = &outgoing.in_reply_to {
        builder = builder.in_reply_to(bracketed(parent));
    }
    if !outgoing.references.is_empty() {
        let ids: Vec<String> = outgoing.references.iter().map(|id| bracketed(id)).collect();
        builder = builder.references(ids.join(" "));
    }

    let text = SinglePart::plain(outgoing.body.clone());
    let message = if outgoing.attachments.is_empty() {
        builder.singlepart(text)
    } else {
        let mut parts = MultiPart::mixed().singlepart(text);
        for path in &outgoing.attachments {
            parts = parts.singlepart(read_attachment(Path::new(path)).await?);
        }
        builder.multipart(parts)
    };
    message.map_err(|e| Error::InvalidInput(format!("メールを組み立てられません: {e}")))
}

fn parse_recipient(recipient: &str) -> Result<Mailbox> {
    recipient
        .trim()
        .parse()
        .map_err(|_| Error::InvalidInput(format!("メールアドレスが正しくありません: {recipient}")))
}

/// Message-ID を `<...>` で囲んだ形にそろえる。
fn bracketed(id: &str) -> String {
    format!("<{}>", id.trim().trim_matches(['<', '>']))
}

async fn read_attachment(path: &Path) -> Result<SinglePart> {
    let filename = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| {
            Error::InvalidInput(format!(
                "添付ファイルのパスが正しくありません: {}",
                path.display()
            ))
        })?;
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|e| Error::InvalidInput(format!("{filename} を読み込めません: {e}")))?;
    let content_type =
        ContentType::parse(mime_guess::from_path(path).first_or_octet_stream().as_ref())
            .unwrap_or(ContentType::parse("application/octet-stream").expect("valid mime literal"));
    Ok(Attachment::new(filename).body(bytes, content_type))
}

fn smtp_error(e: impl std::fmt::Display) -> Error {
    Error::Smtp(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ServerEndpoint;
    use mail_parser::MimeHeaders;

    fn account() -> Account {
        let endpoint = ServerEndpoint {
            host: "mail.example.com".into(),
            port: 465,
            security: Security::Tls,
        };
        Account {
            id: "a1".into(),
            display_name: "山田 太郎".into(),
            email: "yamada@example.com".into(),
            username: "yamada".into(),
            imap: endpoint.clone(),
            smtp: endpoint,
        }
    }

    fn outgoing() -> OutgoingMessage {
        OutgoingMessage {
            to: vec!["Bob <bob@example.com>".into()],
            cc: vec![],
            bcc: vec!["secret@example.com".into()],
            subject: "ご連絡".into(),
            body: "こんにちは".into(),
            attachments: vec![],
            in_reply_to: Some("parent@example.com".into()),
            references: vec!["root@example.com".into(), "<parent@example.com>".into()],
            replied_to: None,
        }
    }

    #[tokio::test]
    async fn builds_a_threaded_reply_without_leaking_bcc() {
        let raw = build_message(&account(), &outgoing())
            .await
            .unwrap()
            .formatted();
        let parsed = mail_parser::MessageParser::default().parse(&raw).unwrap();

        assert_eq!(parsed.subject(), Some("ご連絡"));
        assert_eq!(
            parsed.from().unwrap().first().unwrap().name(),
            Some("山田 太郎")
        );
        assert_eq!(parsed.in_reply_to().as_text(), Some("parent@example.com"));
        assert_eq!(
            parsed.references().as_text_list().unwrap(),
            ["root@example.com", "parent@example.com"]
        );
        assert!(parsed.message_id().is_some());
        assert_eq!(parsed.body_text(0).unwrap().trim(), "こんにちは");
        assert!(!String::from_utf8_lossy(&raw).contains("secret@example.com"));
    }

    #[tokio::test]
    async fn attaches_files_with_a_guessed_content_type() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("メモ.txt");
        std::fs::write(&path, "memo").unwrap();
        let mut message = outgoing();
        message.attachments = vec![path.to_string_lossy().into_owned()];

        let raw = build_message(&account(), &message)
            .await
            .unwrap()
            .formatted();
        let parsed = mail_parser::MessageParser::default().parse(&raw).unwrap();
        let attachment = parsed.attachment(0).unwrap();

        assert_eq!(attachment.attachment_name(), Some("メモ.txt"));
        assert_eq!(attachment.contents(), b"memo");
        assert!(attachment.is_content_type("text", "plain"));
    }

    #[tokio::test]
    async fn rejects_missing_or_invalid_recipients() {
        let mut message = outgoing();
        message.to = vec!["not an address".into()];
        assert!(build_message(&account(), &message).await.is_err());

        message.to.clear();
        message.bcc.clear();
        assert!(build_message(&account(), &message).await.is_err());
    }
}
