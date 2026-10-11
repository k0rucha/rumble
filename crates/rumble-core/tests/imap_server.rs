//! 本物の IMAP サーバー (Dovecot) を相手にした統合テスト。
//!
//! 通常の `cargo test` では実行されない。手順:
//!
//! ```sh
//! docker run -d --rm --name rumble-test-imap -e USER_PASSWORD=password \
//!     -p 31993:31993 dovecot/dovecot:latest
//! cargo test -p rumble-core --test imap_server -- --ignored
//! ```
//!
//! この Dovecot イメージは「任意のユーザー名 + `USER_PASSWORD`」でログインでき、
//! 証明書は自己署名。そのためここでは証明書の検証を無効にして接続する
//! (アプリ本体の接続経路 `open_session` は常に検証する)。

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_native_tls::TlsConnector;
use rumble_core::domain::{Category, MailboxRole, MessageFlag};
use rumble_core::imap::{ops, watcher, ImapSession};
use rumble_core::mime;
use tokio::net::TcpStream;

const SERVER: (&str, u16) = ("127.0.0.1", 31993);

async fn login(user: &str) -> ImapSession {
    let tcp = TcpStream::connect(SERVER)
        .await
        .expect("Dovecot が起動していません");
    let tls = TlsConnector::new().danger_accept_invalid_certs(true);
    let mut client = async_imap::Client::new(tls.connect(SERVER.0, tcp).await.unwrap());
    client.read_response().await.unwrap();
    client
        .login(user, "password")
        .await
        .map_err(|(e, _)| e)
        .unwrap()
}

/// テストごとに別ユーザー (= 空のメールボックス) を使う。
fn fresh_user(test: &str) -> String {
    format!("{test}-{}", std::process::id())
}

fn mail(subject_header: &str, body: &str) -> Vec<u8> {
    format!(
        "From: Alice <alice@example.com>\r\nTo: bob@example.com\r\nSubject: {subject_header}\r\n\
         Date: Sat, 10 Oct 2026 12:00:00 +0900\r\nMIME-Version: 1.0\r\n\
         Content-Type: text/plain; charset=utf-8\r\n\r\n{body}\r\n"
    )
    .into_bytes()
}

#[tokio::test]
#[ignore = "要 Dovecot (ファイル冒頭を参照)"]
async fn browse_search_and_organize_mail_on_the_server() {
    let mut session = login(&fresh_user("ops")).await;
    let s = &mut session;

    // この Dovecot は INBOX しか用意しないので、よくある名前でフォルダを作っておく
    s.create("Trash").await.unwrap();
    s.create("Sent").await.unwrap();

    // --- フォルダ一覧: 役割が判別できる
    let mailboxes = ops::list_mailboxes(s).await.unwrap();
    let role_of = |role| {
        mailboxes
            .iter()
            .find(|m| m.role == role)
            .map(|m| m.path.clone())
    };
    assert_eq!(role_of(MailboxRole::Inbox).as_deref(), Some("INBOX"));
    let trash = role_of(MailboxRole::Trash).expect("Trash フォルダ");
    let sent = role_of(MailboxRole::Sent).expect("Sent フォルダ");

    // --- 3 通を置く (APPEND は送信済みの控えにも使う経路)
    ops::append_message(s, "INBOX", &mail("first", "one"))
        .await
        .unwrap();
    ops::append_message(
        s,
        "INBOX",
        &mail("=?UTF-8?B?6KuL5rGC5pu4?=", "今月の請求書です"),
    )
    .await
    .unwrap();
    ops::append_message(s, "INBOX", &mail("third", "three"))
        .await
        .unwrap();

    // --- 一覧: 新しい順にページ単位で取れる。本文は取らない
    let page = ops::list_messages(s, "INBOX", 0, 2).await.unwrap();
    assert_eq!(page.total, 3);
    let subjects: Vec<&str> = page.messages.iter().map(|m| m.subject.as_str()).collect();
    assert_eq!(subjects, ["third", "請求書"]);
    assert_eq!(
        page.messages[0].from.as_ref().unwrap().email,
        "alice@example.com"
    );
    assert_eq!(page.messages[0].date, Some(1_791_601_200));
    assert!(page.messages[0].seen);

    let rest = ops::list_messages(s, "INBOX", 2, 2).await.unwrap();
    assert_eq!(rest.messages.len(), 1);
    assert_eq!(rest.messages[0].subject, "first");
    assert!(ops::list_messages(s, "INBOX", 3, 2)
        .await
        .unwrap()
        .messages
        .is_empty());

    let (third, invoice, first) = (
        page.messages[0].uid,
        page.messages[1].uid,
        rest.messages[0].uid,
    );

    // --- 検索: サーバー側で日本語の本文を探せる
    let found = ops::search_messages(s, "INBOX", "請求書").await.unwrap();
    assert_eq!(found.total, 1);
    assert_eq!(found.messages[0].uid, invoice);
    assert_eq!(
        ops::search_messages(s, "INBOX", "nothing-matches")
            .await
            .unwrap()
            .total,
        0
    );

    // --- 本文取得: 開いても既読フラグは勝手に変わらない
    ops::set_flag(s, "INBOX", &[invoice], MessageFlag::Seen, false)
        .await
        .unwrap();
    let raw = ops::fetch_raw_message(s, "INBOX", invoice).await.unwrap();
    let body = mime::parse_message(invoice, &raw).unwrap();
    assert_eq!(body.subject, "請求書");
    assert_eq!(
        body.text.as_deref().map(str::trim),
        Some("今月の請求書です")
    );
    assert_eq!(ops::mailbox_status(s, "INBOX").await.unwrap().unseen, 1);
    assert!(ops::fetch_raw_message(s, "INBOX", 9999).await.is_err());

    // --- フラグ
    ops::set_flag(s, "INBOX", &[invoice], MessageFlag::Seen, true)
        .await
        .unwrap();
    ops::set_flag(s, "INBOX", &[invoice], MessageFlag::Flagged, true)
        .await
        .unwrap();
    let flagged = &ops::list_messages(s, "INBOX", 0, 50)
        .await
        .unwrap()
        .messages[1];
    assert!(flagged.seen && flagged.flagged && !flagged.answered);
    assert_eq!(ops::mailbox_status(s, "INBOX").await.unwrap().unseen, 0);

    // --- 移動
    ops::move_messages(s, "INBOX", &[first], &sent)
        .await
        .unwrap();
    assert_eq!(
        ops::list_messages(s, "INBOX", 0, 50).await.unwrap().total,
        2
    );
    assert_eq!(
        ops::list_messages(s, &sent, 0, 50).await.unwrap().messages[0].subject,
        "first"
    );

    // --- 削除: まずゴミ箱へ、ゴミ箱の中では完全に消える
    ops::delete_messages(s, "INBOX", &[third]).await.unwrap();
    assert_eq!(
        ops::list_messages(s, "INBOX", 0, 50).await.unwrap().total,
        1
    );
    let in_trash = ops::list_messages(s, &trash, 0, 50).await.unwrap();
    assert_eq!(in_trash.messages[0].subject, "third");

    ops::delete_messages(s, &trash, &[in_trash.messages[0].uid])
        .await
        .unwrap();
    assert_eq!(ops::list_messages(s, &trash, 0, 50).await.unwrap().total, 0);
    // 残った 1 通は巻き添えになっていない
    assert_eq!(
        ops::list_messages(s, "INBOX", 0, 50)
            .await
            .unwrap()
            .messages[0]
            .uid,
        invoice
    );
}

#[tokio::test]
#[ignore = "要 Dovecot (ファイル冒頭を参照)"]
async fn categories_live_on_the_server_as_keywords() {
    let user = fresh_user("category");
    let mut session = login(&user).await;
    let s = &mut session;
    // 件名の後ろに、一斉配信メールが持つヘッダーを足す
    let bulk_subject = "Autumn sale\r\nList-Unsubscribe: <https://shop.example/unsubscribe>";
    ops::append_message(s, "INBOX", &mail(bulk_subject, "50% off"))
        .await
        .unwrap();
    ops::append_message(s, "INBOX", &mail("Lunch?", "tomorrow"))
        .await
        .unwrap();
    let page = ops::list_messages(s, "INBOX", 0, 50).await.unwrap();
    let (lunch, sale) = (page.messages[0].uid, page.messages[1].uid);
    assert!(page.messages.iter().all(|m| m.category.is_none()));

    // --- 分類の手がかり: ヘッダーだけで取れる
    let mut signals = ops::fetch_classification_signals(s, "INBOX", &[sale, lunch])
        .await
        .unwrap();
    signals.sort_by_key(|(uid, _)| *uid);
    assert_eq!(signals[0].0, sale);
    assert_eq!(signals[0].1.subject, "Autumn sale");
    assert!(signals[0].1.is_mailing_list);
    assert!(!signals[1].1.is_mailing_list);

    // --- 結果をキーワードとして保存すると、一覧と一緒に返ってくる
    let classified = [(sale, Category::Newsletter), (lunch, Category::Personal)];
    assert!(ops::store_categories(s, "INBOX", &classified)
        .await
        .unwrap());
    let page = ops::list_messages(s, "INBOX", 0, 50).await.unwrap();
    assert_eq!(page.messages[0].category, Some(Category::Personal));
    assert_eq!(page.messages[1].category, Some(Category::Newsletter));

    // --- 区分での絞り込みはサーバー側のキーワード検索
    let newsletters = ops::list_by_category(s, "INBOX", Category::Newsletter)
        .await
        .unwrap();
    assert_eq!(newsletters.total, 1);
    assert_eq!(newsletters.messages[0].uid, sale);
    assert_eq!(
        ops::list_by_category(s, "INBOX", Category::Receipt)
            .await
            .unwrap()
            .total,
        0
    );

    // --- 別の接続 (= 別の端末) からも同じ分類が見える
    let mut another_device = login(&user).await;
    let seen = ops::list_messages(&mut another_device, "INBOX", 0, 50)
        .await
        .unwrap();
    assert_eq!(seen.messages[1].category, Some(Category::Newsletter));
}

#[tokio::test]
#[ignore = "要 Dovecot (ファイル冒頭を参照)"]
async fn idle_reports_new_mail_without_polling() {
    let user = fresh_user("idle");
    let watching = login(&user).await;
    let mut other_client = login(&user).await;

    let changes = Arc::new(AtomicUsize::new(0));
    let counter = changes.clone();
    let watcher = tokio::spawn(async move {
        let _ = watcher::watch_session(watching, &move || {
            counter.fetch_add(1, Ordering::SeqCst);
        })
        .await;
    });

    tokio::time::sleep(Duration::from_millis(500)).await; // IDLE に入るのを待つ
    assert_eq!(changes.load(Ordering::SeqCst), 0);
    ops::append_message(&mut other_client, "INBOX", &mail("ping", "hello"))
        .await
        .unwrap();

    for _ in 0..50 {
        if changes.load(Ordering::SeqCst) > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    watcher.abort();
    assert!(
        changes.load(Ordering::SeqCst) > 0,
        "IDLE で新着が通知されなかった"
    );
}
