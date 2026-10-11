//! IMAP 操作。
//!
//! どの関数も「ログイン済みのセッションを受け取り、必要なコマンドを発行して
//! 結果を返す」だけで、関数をまたぐ状態を持たない。対象フォルダは毎回 SELECT する
//! (同じフォルダの再 SELECT は安価で、最新の件数も得られる)。
//!
//! メールの特定には、セッションをまたいでも変わらない UID だけを使う。

use async_imap::types::Fetch;
use futures::TryStreamExt;

use super::mapping;
use super::ImapSession;
use crate::domain::{Mailbox, MailboxRole, MailboxStatus, MessageFlag, MessagePage};
use crate::{Error, Result};

/// 一覧に必要な最小限の項目。本文は取らない (PEEK なので既読にもならない)。
const SUMMARY_ITEMS: &str = "(UID FLAGS INTERNALDATE RFC822.SIZE \
    BODY.PEEK[HEADER.FIELDS (FROM TO SUBJECT DATE CONTENT-TYPE)])";

/// 検索結果として一度に返す最大件数 (新しいものを優先)。
const SEARCH_RESULT_LIMIT: usize = 200;

pub async fn list_mailboxes(session: &mut ImapSession) -> Result<Vec<Mailbox>> {
    let names: Vec<_> = session
        .list(Some(""), Some("*"))
        .await?
        .try_collect()
        .await?;
    let mut mailboxes: Vec<Mailbox> = names.iter().map(mapping::mailbox_from_name).collect();
    mapping::sort_mailboxes(&mut mailboxes);
    Ok(mailboxes)
}

pub async fn mailbox_status(session: &mut ImapSession, mailbox: &str) -> Result<MailboxStatus> {
    let status = session.status(mailbox, "(MESSAGES UNSEEN)").await?;
    Ok(MailboxStatus {
        total: status.exists,
        unseen: status.unseen.unwrap_or(0),
    })
}

/// 新しい順に `offset` 件飛ばして `limit` 件。
///
/// サーバー側の並び (シーケンス番号 = 到着順) の末尾から切り出すだけなので、
/// フォルダに何万通あっても取得するのは表示するぶんだけ。
pub async fn list_messages(
    session: &mut ImapSession,
    mailbox: &str,
    offset: u32,
    limit: u32,
) -> Result<MessagePage> {
    let total = session.select(mailbox).await?.exists;
    if offset >= total || limit == 0 {
        return Ok(MessagePage {
            total,
            messages: Vec::new(),
        });
    }
    let newest = total - offset;
    let oldest = newest.saturating_sub(limit - 1).max(1);

    let fetches: Vec<Fetch> = session
        .fetch(format!("{oldest}:{newest}"), SUMMARY_ITEMS)
        .await?
        .try_collect()
        .await?;
    Ok(MessagePage {
        total,
        messages: newest_first(&fetches),
    })
}

/// サーバー側で全文検索する (ヘッダーと本文が対象)。
///
/// ローカルに索引を持たないので、検索は常にサーバーの仕事。
pub async fn search_messages(
    session: &mut ImapSession,
    mailbox: &str,
    query: &str,
) -> Result<MessagePage> {
    session.select(mailbox).await?;
    let mut uids: Vec<u32> = session
        .uid_search(search_criteria(query)?)
        .await?
        .into_iter()
        .collect();
    let total = uids.len() as u32;
    if uids.is_empty() {
        return Ok(MessagePage {
            total,
            messages: Vec::new(),
        });
    }
    uids.sort_unstable_by_key(|&uid| std::cmp::Reverse(uid));
    uids.truncate(SEARCH_RESULT_LIMIT);

    let fetches: Vec<Fetch> = session
        .uid_fetch(uid_set(&uids), SUMMARY_ITEMS)
        .await?
        .try_collect()
        .await?;
    Ok(MessagePage {
        total,
        messages: newest_first(&fetches),
    })
}

/// メール 1 通をまるごと取得する。PEEK なので既読フラグは変えない
/// (既読にするかどうかは利用者の操作として別に扱う)。
pub async fn fetch_raw_message(
    session: &mut ImapSession,
    mailbox: &str,
    uid: u32,
) -> Result<Vec<u8>> {
    session.select(mailbox).await?;
    let fetches: Vec<Fetch> = session
        .uid_fetch(uid.to_string(), "(UID BODY.PEEK[])")
        .await?
        .try_collect()
        .await?;
    fetches
        .iter()
        .find(|fetch| fetch.uid == Some(uid))
        .and_then(|fetch| fetch.body())
        .map(<[u8]>::to_vec)
        .ok_or_else(|| {
            Error::NotFound("メールが見つかりません。すでに移動または削除されています".into())
        })
}

pub async fn set_flag(
    session: &mut ImapSession,
    mailbox: &str,
    uids: &[u32],
    flag: MessageFlag,
    enabled: bool,
) -> Result<()> {
    if uids.is_empty() {
        return Ok(());
    }
    session.select(mailbox).await?;
    let sign = if enabled { '+' } else { '-' };
    store(
        session,
        uids,
        &format!("{sign}FLAGS.SILENT ({})", flag.as_imap()),
    )
    .await
}

pub async fn move_messages(
    session: &mut ImapSession,
    mailbox: &str,
    uids: &[u32],
    destination: &str,
) -> Result<()> {
    if uids.is_empty() {
        return Ok(());
    }
    session.select(mailbox).await?;
    if session.capabilities().await?.has_str("MOVE") {
        session.uid_mv(uid_set(uids), destination).await?;
    } else {
        // MOVE 拡張の無いサーバーでは「コピーして元を消す」
        session.uid_copy(uid_set(uids), destination).await?;
        expunge_uids(session, uids).await?;
    }
    Ok(())
}

/// 削除する。ゴミ箱があればそこへ移動し、ゴミ箱の中 (またはゴミ箱が無い
/// サーバー) では完全に消す。
pub async fn delete_messages(session: &mut ImapSession, mailbox: &str, uids: &[u32]) -> Result<()> {
    if uids.is_empty() {
        return Ok(());
    }
    let trash = list_mailboxes(session)
        .await?
        .into_iter()
        .find(|candidate| candidate.role == MailboxRole::Trash && candidate.selectable);

    match trash {
        Some(trash) if trash.path != mailbox => {
            move_messages(session, mailbox, uids, &trash.path).await
        }
        _ => {
            session.select(mailbox).await?;
            expunge_uids(session, uids).await
        }
    }
}

/// 送信したメールの控えをフォルダへ追加する (既読として)。
pub async fn append_message(session: &mut ImapSession, mailbox: &str, raw: &[u8]) -> Result<()> {
    session.append(mailbox, Some("(\\Seen)"), None, raw).await?;
    Ok(())
}

/// Gmail は送信時にサーバー側で「送信済み」へ保存するので、控えの追加が不要。
pub async fn saves_sent_mail_itself(session: &mut ImapSession) -> Result<bool> {
    Ok(session.capabilities().await?.has_str("X-GM-EXT-1"))
}

/// SELECT 済みのフォルダから、指定した UID だけを完全に削除する。
async fn expunge_uids(session: &mut ImapSession, uids: &[u32]) -> Result<()> {
    store(session, uids, "+FLAGS.SILENT (\\Deleted)").await?;
    if session.capabilities().await?.has_str("UIDPLUS") {
        session
            .uid_expunge(uid_set(uids))
            .await?
            .try_collect::<Vec<_>>()
            .await?;
    } else {
        // UIDPLUS が無いと対象を絞れず、\Deleted の付いた全メールが消える。
        // 他のクライアントが削除マークだけ付けたメールも巻き込む点に注意。
        session.expunge().await?.try_collect::<Vec<_>>().await?;
    }
    Ok(())
}

async fn store(session: &mut ImapSession, uids: &[u32], change: &str) -> Result<()> {
    // 応答ストリームを最後まで読まないとコマンドが完了しない
    session
        .uid_store(uid_set(uids), change)
        .await?
        .try_collect::<Vec<_>>()
        .await?;
    Ok(())
}

fn newest_first(fetches: &[Fetch]) -> Vec<crate::domain::MessageSummary> {
    let mut messages: Vec<_> = fetches
        .iter()
        .filter_map(mapping::summary_from_fetch)
        .collect();
    // UID は到着順に増えるので、降順 = 新しい順
    messages.sort_unstable_by_key(|message| std::cmp::Reverse(message.uid));
    messages.dedup_by_key(|message| message.uid);
    messages
}

fn uid_set(uids: &[u32]) -> String {
    uids.iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

/// 検索語を IMAP の SEARCH 条件にする。
fn search_criteria(query: &str) -> Result<String> {
    let query = query.trim();
    if query.is_empty() || query.contains(['\r', '\n']) {
        return Err(Error::InvalidInput("検索語が正しくありません".into()));
    }
    let quoted = format!("\"{}\"", query.replace('\\', "\\\\").replace('"', "\\\""));
    Ok(if query.is_ascii() {
        format!("TEXT {quoted}")
    } else {
        // 日本語などは文字コードを明示する
        format!("CHARSET UTF-8 TEXT {quoted}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_uid_sets() {
        assert_eq!(uid_set(&[3, 10, 7]), "3,10,7");
    }

    #[test]
    fn quotes_search_terms() {
        assert_eq!(search_criteria(" invoice ").unwrap(), "TEXT \"invoice\"");
        assert_eq!(
            search_criteria(r#"say "hi"\"#).unwrap(),
            r#"TEXT "say \"hi\"\\""#
        );
    }

    #[test]
    fn declares_charset_for_non_ascii_terms() {
        assert_eq!(
            search_criteria("請求書").unwrap(),
            "CHARSET UTF-8 TEXT \"請求書\""
        );
    }

    #[test]
    fn rejects_empty_or_multiline_terms() {
        assert!(search_criteria("   ").is_err());
        assert!(search_criteria("a\r\nA1 LOGOUT").is_err());
    }
}
