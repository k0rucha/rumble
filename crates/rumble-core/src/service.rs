//! ユースケース層。画面からの要求はすべてここを通る。
//!
//! [`MailService`] の公開メソッド 1 つが、利用者の操作 1 つに対応する。
//! 各メソッドは「誰の (account_id)・どのフォルダの (mailbox)・どのメール (uid)」を
//! 引数で受け取るだけで、選択中のフォルダのような画面の状態は持たない。

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};

use tokio::sync::Mutex as AsyncMutex;

use crate::cache::{CacheKey, MessageCache};
use crate::domain::{
    Account, MailEvent, Mailbox, MailboxRole, MailboxStatus, MessageBody, MessageFlag, MessagePage,
    NewAccount, OutgoingMessage, SendReport, SentCopy,
};
use crate::imap::watcher::InboxWatcher;
use crate::imap::{connection, ops, ImapConnection, ImapSession};
use crate::storage::{AccountStore, SecretStore};
use crate::{mime, smtp, Error, Result};

/// 開いたメールをメモリに保持する上限。超えたぶんは古い順に捨てる。
const MESSAGE_CACHE_BYTES: usize = 64 * 1024 * 1024;

/// サーバー側の変化を画面へ伝えるためのコールバック。
pub type EventSink = Arc<dyn Fn(MailEvent) + Send + Sync>;

pub struct MailService {
    account_store: Box<dyn AccountStore>,
    secret_store: Box<dyn SecretStore>,
    events: EventSink,

    accounts: RwLock<Vec<Account>>,
    /// アカウントごとの操作用 IMAP 接続。操作は 1 接続につき 1 つずつ直列に流す。
    connections: Mutex<HashMap<String, Arc<AsyncMutex<ImapConnection>>>>,
    watchers: Mutex<HashMap<String, InboxWatcher>>,
    cache: Mutex<MessageCache>,
}

impl MailService {
    pub fn new(
        account_store: Box<dyn AccountStore>,
        secret_store: Box<dyn SecretStore>,
        events: EventSink,
    ) -> Result<Self> {
        let accounts = account_store.load()?;
        Ok(Self {
            account_store,
            secret_store,
            events,
            accounts: RwLock::new(accounts),
            connections: Mutex::default(),
            watchers: Mutex::default(),
            cache: Mutex::new(MessageCache::new(MESSAGE_CACHE_BYTES)),
        })
    }

    // ---- アカウント -------------------------------------------------------

    pub fn list_accounts(&self) -> Vec<Account> {
        self.accounts.read().unwrap().clone()
    }

    /// アカウントを追加する。保存する前に、実際に IMAP / SMTP へログインして確かめる。
    pub async fn add_account(&self, input: NewAccount) -> Result<Account> {
        let account = Account {
            id: uuid::Uuid::new_v4().to_string(),
            display_name: input.display_name.trim().to_string(),
            email: input.email.trim().to_string(),
            username: input.username.trim().to_string(),
            imap: input.imap,
            smtp: input.smtp,
        };
        let mut session =
            connection::open_session(&account.imap, &account.username, &input.password).await?;
        let _ = session.logout().await;
        smtp::verify(&account, &input.password).await?;

        self.secret_store.set(&account.id, &input.password)?;
        let mut accounts = self.list_accounts();
        accounts.push(account.clone());
        self.replace_accounts(accounts)?;

        self.start_watcher(&account, input.password);
        Ok(account)
    }

    /// アカウントを rumble から外す。サーバー上のメールには何もしない。
    pub fn remove_account(&self, account_id: &str) -> Result<()> {
        let mut accounts = self.list_accounts();
        accounts.retain(|account| account.id != account_id);
        self.replace_accounts(accounts)?;

        self.watchers.lock().unwrap().remove(account_id);
        self.connections.lock().unwrap().remove(account_id);
        self.cache.lock().unwrap().remove_account(account_id);
        self.secret_store.delete(account_id)
    }

    /// 登録済みの全アカウントで受信トレイの監視を始める (起動時に 1 回)。
    /// tokio ランタイム上で呼ぶこと。
    pub fn start_watchers(&self) {
        for account in self.list_accounts() {
            // パスワードを読めないアカウントは監視しないだけ。操作時に改めてエラーを出す
            if let Ok(password) = self.secret_store.get(&account.id) {
                self.start_watcher(&account, password);
            }
        }
    }

    // ---- フォルダ ---------------------------------------------------------

    pub async fn list_mailboxes(&self, account_id: &str) -> Result<Vec<Mailbox>> {
        self.imap(account_id, async |session| {
            ops::list_mailboxes(session).await
        })
        .await
    }

    pub async fn mailbox_status(&self, account_id: &str, mailbox: &str) -> Result<MailboxStatus> {
        self.imap(account_id, async |session| {
            ops::mailbox_status(session, mailbox).await
        })
        .await
    }

    // ---- 一覧と検索 -------------------------------------------------------

    pub async fn list_messages(
        &self,
        account_id: &str,
        mailbox: &str,
        offset: u32,
        limit: u32,
    ) -> Result<MessagePage> {
        self.imap(account_id, async |session| {
            ops::list_messages(session, mailbox, offset, limit).await
        })
        .await
    }

    pub async fn search_messages(
        &self,
        account_id: &str,
        mailbox: &str,
        query: &str,
    ) -> Result<MessagePage> {
        self.imap(account_id, async |session| {
            ops::search_messages(session, mailbox, query).await
        })
        .await
    }

    // ---- メールを開く -----------------------------------------------------

    pub async fn get_message(
        &self,
        account_id: &str,
        mailbox: &str,
        uid: u32,
    ) -> Result<MessageBody> {
        let raw = self.raw_message(account_id, mailbox, uid).await?;
        mime::parse_message(uid, &raw)
    }

    /// 添付ファイルを `destination` に書き出す。
    ///
    /// rumble がメールの中身をディスクに書くのは、利用者が保存先を選んだこの場合だけ。
    pub async fn save_attachment(
        &self,
        account_id: &str,
        mailbox: &str,
        uid: u32,
        attachment_index: u32,
        destination: &Path,
    ) -> Result<()> {
        let raw = self.raw_message(account_id, mailbox, uid).await?;
        let (_filename, bytes) = mime::extract_attachment(&raw, attachment_index)?;
        tokio::fs::write(destination, bytes).await?;
        Ok(())
    }

    // ---- メールの操作 -----------------------------------------------------

    pub async fn set_flag(
        &self,
        account_id: &str,
        mailbox: &str,
        uids: &[u32],
        flag: MessageFlag,
        enabled: bool,
    ) -> Result<()> {
        self.imap(account_id, async |session| {
            ops::set_flag(session, mailbox, uids, flag, enabled).await
        })
        .await
    }

    pub async fn move_messages(
        &self,
        account_id: &str,
        mailbox: &str,
        uids: &[u32],
        destination: &str,
    ) -> Result<()> {
        self.imap(account_id, async |session| {
            ops::move_messages(session, mailbox, uids, destination).await
        })
        .await?;
        self.forget_cached(account_id, mailbox, uids);
        Ok(())
    }

    /// ゴミ箱へ移動する。ゴミ箱の中では完全に削除する。
    pub async fn delete_messages(
        &self,
        account_id: &str,
        mailbox: &str,
        uids: &[u32],
    ) -> Result<()> {
        self.imap(account_id, async |session| {
            ops::delete_messages(session, mailbox, uids).await
        })
        .await?;
        self.forget_cached(account_id, mailbox, uids);
        Ok(())
    }

    // ---- 送信 -------------------------------------------------------------

    /// 送信する。成功したら後始末として
    /// 1. 返信元に「返信済み」フラグを立てる
    /// 2. 「送信済み」フォルダへ控えを置く (サーバーが自動でやる場合を除く)
    ///
    /// 後始末の失敗では送信を失敗扱いにしない (メールはもう相手に届いている)。
    pub async fn send_message(
        &self,
        account_id: &str,
        outgoing: OutgoingMessage,
    ) -> Result<SendReport> {
        let account = self.account(account_id)?;
        let password = self.secret_store.get(account_id)?;
        let raw = smtp::send(&account, &password, &outgoing).await?;

        if let Some(original) = &outgoing.replied_to {
            let _ = self
                .set_flag(
                    account_id,
                    &original.mailbox,
                    &[original.uid],
                    MessageFlag::Answered,
                    true,
                )
                .await;
        }
        let sent_copy = match self.keep_sent_copy(account_id, &raw).await {
            Ok(outcome) => outcome,
            Err(error) => SentCopy::Failed(error.to_string()),
        };
        Ok(SendReport { sent_copy })
    }

    async fn keep_sent_copy(&self, account_id: &str, raw: &[u8]) -> Result<SentCopy> {
        self.imap(account_id, async |session| {
            if ops::saves_sent_mail_itself(session).await? {
                return Ok(SentCopy::ServerManaged);
            }
            let sent = ops::list_mailboxes(session)
                .await?
                .into_iter()
                .find(|mailbox| mailbox.role == MailboxRole::Sent && mailbox.selectable)
                .ok_or_else(|| Error::NotFound("送信済みフォルダが見つかりません".into()))?;
            ops::append_message(session, &sent.path, raw).await?;
            Ok(SentCopy::Appended)
        })
        .await
    }

    // ---- 内部 -------------------------------------------------------------

    fn account(&self, account_id: &str) -> Result<Account> {
        self.accounts
            .read()
            .unwrap()
            .iter()
            .find(|account| account.id == account_id)
            .cloned()
            .ok_or_else(|| Error::NotFound("アカウントが見つかりません".into()))
    }

    /// 先にファイルへ保存し、成功したらメモリ上の一覧も入れ替える。
    fn replace_accounts(&self, accounts: Vec<Account>) -> Result<()> {
        self.account_store.save(&accounts)?;
        *self.accounts.write().unwrap() = accounts;
        Ok(())
    }

    fn start_watcher(&self, account: &Account, password: String) {
        let events = self.events.clone();
        let account_id = account.id.clone();
        let watcher = InboxWatcher::spawn(account.clone(), password, move || {
            events(MailEvent::MailboxChanged {
                account_id: account_id.clone(),
                mailbox: "INBOX".to_string(),
            })
        });
        self.watchers
            .lock()
            .unwrap()
            .insert(account.id.clone(), watcher);
    }

    /// そのアカウントの IMAP セッション上で `operation` を実行する。
    /// 接続・再接続・タイムアウトは [`ImapConnection::run`] が面倒を見る。
    async fn imap<T>(
        &self,
        account_id: &str,
        operation: impl AsyncFnOnce(&mut ImapSession) -> Result<T>,
    ) -> Result<T> {
        let connection = self.connection(account_id)?;
        let mut connection = connection.lock().await;
        connection.run(operation).await
    }

    fn connection(&self, account_id: &str) -> Result<Arc<AsyncMutex<ImapConnection>>> {
        if let Some(existing) = self.connections.lock().unwrap().get(account_id) {
            return Ok(existing.clone());
        }
        let account = self.account(account_id)?;
        let password = self.secret_store.get(account_id)?;
        let connection = Arc::new(AsyncMutex::new(ImapConnection::new(&account, password)));
        Ok(self
            .connections
            .lock()
            .unwrap()
            .entry(account_id.to_string())
            .or_insert(connection)
            .clone())
    }

    /// メール 1 通の生データ。メモリにあればそれを、無ければサーバーから取る。
    async fn raw_message(&self, account_id: &str, mailbox: &str, uid: u32) -> Result<Arc<Vec<u8>>> {
        let key = CacheKey {
            account_id: account_id.to_string(),
            mailbox: mailbox.to_string(),
            uid,
        };
        if let Some(raw) = self.cache.lock().unwrap().get(&key) {
            return Ok(raw);
        }
        let raw = Arc::new(
            self.imap(account_id, async |session| {
                ops::fetch_raw_message(session, mailbox, uid).await
            })
            .await?,
        );
        self.cache.lock().unwrap().insert(key, raw.clone());
        Ok(raw)
    }

    fn forget_cached(&self, account_id: &str, mailbox: &str, uids: &[u32]) {
        let mut cache = self.cache.lock().unwrap();
        for &uid in uids {
            cache.remove(&CacheKey {
                account_id: account_id.to_string(),
                mailbox: mailbox.to_string(),
                uid,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tauri のコマンドは別スレッドで実行されるので、公開メソッドの Future は
    /// Send でなければならない。崩れたらここでコンパイルエラーになる。
    #[allow(dead_code)]
    fn public_futures_are_send(service: &'static MailService) {
        fn assert_send<T: Send>(_: T) {}
        assert_send(service.list_mailboxes("a"));
        assert_send(service.list_messages("a", "INBOX", 0, 50));
        assert_send(service.search_messages("a", "INBOX", "q"));
        assert_send(service.get_message("a", "INBOX", 1));
        assert_send(service.set_flag("a", "INBOX", &[1], MessageFlag::Seen, true));
        assert_send(service.move_messages("a", "INBOX", &[1], "Archive"));
        assert_send(service.delete_messages("a", "INBOX", &[1]));
        assert_send(service.save_attachment("a", "INBOX", 1, 0, Path::new("/tmp/x")));
    }
}
