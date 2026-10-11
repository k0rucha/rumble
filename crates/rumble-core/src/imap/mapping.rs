//! IMAP の応答をドメイン型へ変換する。ネットワークには触れない。

use async_imap::types::{Fetch, Flag, Name, NameAttribute};

use crate::domain::{Mailbox, MailboxRole, MessageSummary};
use crate::mime;

pub fn mailbox_from_name(name: &Name) -> Mailbox {
    let attributes = name.attributes();
    let special_use = attributes.iter().find_map(|attribute| match attribute {
        NameAttribute::All => Some(MailboxRole::All),
        NameAttribute::Archive => Some(MailboxRole::Archive),
        NameAttribute::Drafts => Some(MailboxRole::Drafts),
        NameAttribute::Flagged => Some(MailboxRole::Flagged),
        NameAttribute::Junk => Some(MailboxRole::Junk),
        NameAttribute::Sent => Some(MailboxRole::Sent),
        NameAttribute::Trash => Some(MailboxRole::Trash),
        _ => None,
    });
    let selectable = !attributes
        .iter()
        .any(|a| matches!(a, NameAttribute::NoSelect));
    build_mailbox(name.name(), name.delimiter(), special_use, selectable)
}

fn build_mailbox(
    path: &str,
    delimiter: Option<&str>,
    special_use: Option<MailboxRole>,
    selectable: bool,
) -> Mailbox {
    // フォルダ名は modified UTF-7 (RFC 3501) で届く。表示用にだけデコードする
    let decoded = utf7_imap::decode_utf7_imap(path.to_string());
    let segments: Vec<&str> = match delimiter {
        Some(delimiter) if !delimiter.is_empty() => decoded.split(delimiter).collect(),
        _ => vec![decoded.as_str()],
    };
    let name = segments.last().copied().unwrap_or_default().to_string();

    Mailbox {
        path: path.to_string(),
        depth: segments.len() - 1,
        role: special_use.unwrap_or_else(|| guess_role(&decoded)),
        name,
        selectable,
    }
}

/// SPECIAL-USE を返さないサーバー向けに、よくある名前から役割を推測する。
fn guess_role(decoded_path: &str) -> MailboxRole {
    match decoded_path.to_ascii_lowercase().as_str() {
        "inbox" => MailboxRole::Inbox,
        "sent" | "sent items" | "sent messages" | "sent mail" | "inbox.sent" => MailboxRole::Sent,
        "drafts" | "draft" | "inbox.drafts" => MailboxRole::Drafts,
        "trash" | "deleted items" | "deleted messages" | "inbox.trash" => MailboxRole::Trash,
        "junk" | "spam" | "junk e-mail" | "junk email" | "inbox.junk" => MailboxRole::Junk,
        "archive" | "archives" | "inbox.archive" => MailboxRole::Archive,
        _ => MailboxRole::Other,
    }
}

/// サイドバーの表示順: 役割のあるフォルダを先に、残りはパス順。
pub fn sort_mailboxes(mailboxes: &mut [Mailbox]) {
    mailboxes.sort_by(|a, b| {
        (
            a.role,
            utf7_imap::decode_utf7_imap(a.path.clone()).to_lowercase(),
        )
            .cmp(&(
                b.role,
                utf7_imap::decode_utf7_imap(b.path.clone()).to_lowercase(),
            ))
    });
}

/// 一覧用 FETCH の 1 件を変換する。UID の無い応答 (他セッション起因の通知など) は捨てる。
pub fn summary_from_fetch(fetch: &Fetch) -> Option<MessageSummary> {
    let uid = fetch.uid?;
    let headers = fetch
        .header()
        .map(mime::parse_summary_headers)
        .unwrap_or_default();
    let has_flag = |wanted: Flag<'static>| fetch.flags().any(|flag| flag == wanted);

    Some(MessageSummary {
        uid,
        subject: headers.subject,
        from: headers.from,
        to: headers.to,
        date: headers
            .date
            .or_else(|| fetch.internal_date().map(|d| d.timestamp())),
        size: fetch.size.unwrap_or(0),
        seen: has_flag(Flag::Seen),
        flagged: has_flag(Flag::Flagged),
        answered: has_flag(Flag::Answered),
        has_attachment: headers.has_attachment,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_japanese_folder_names_but_keeps_raw_path() {
        let mailbox = build_mailbox("&U9dP4TDIMOwwpA-/&TtVOiw-", Some("/"), None, true);
        assert_eq!(mailbox.path, "&U9dP4TDIMOwwpA-/&TtVOiw-");
        assert_eq!(mailbox.name, "仕事");
        assert_eq!(mailbox.depth, 1);
        assert_eq!(mailbox.role, MailboxRole::Other);
    }

    #[test]
    fn special_use_attribute_wins_over_name() {
        // Gmail の日本語環境: 名前からは分からないが属性で送信済みと分かる
        let mailbox = build_mailbox(
            "[Gmail]/&kAFP4W4IMH8w4TD8MOs-",
            Some("/"),
            Some(MailboxRole::Sent),
            true,
        );
        assert_eq!(mailbox.role, MailboxRole::Sent);
        assert_eq!(mailbox.name, "送信済みメール");
    }

    #[test]
    fn guesses_role_from_common_names() {
        assert_eq!(
            build_mailbox("INBOX", Some("/"), None, true).role,
            MailboxRole::Inbox
        );
        assert_eq!(
            build_mailbox("Sent Items", Some("/"), None, true).role,
            MailboxRole::Sent
        );
        assert_eq!(
            build_mailbox("Projects", Some("/"), None, true).role,
            MailboxRole::Other
        );
    }

    #[test]
    fn sorts_special_folders_first() {
        let mut mailboxes = vec![
            build_mailbox("Zeta", Some("/"), None, true),
            build_mailbox("Trash", Some("/"), None, true),
            build_mailbox("alpha", Some("/"), None, true),
            build_mailbox("INBOX", Some("/"), None, true),
        ];
        sort_mailboxes(&mut mailboxes);
        let order: Vec<&str> = mailboxes.iter().map(|m| m.path.as_str()).collect();
        assert_eq!(order, ["INBOX", "Trash", "alpha", "Zeta"]);
    }
}
