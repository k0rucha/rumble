//! IDLE (RFC 2177) による受信トレイの監視。
//!
//! 操作用とは別の接続を 1 本張り、サーバーからの「何か変わった」という通知を
//! 待ち続ける。定期的に問い合わせるポーリングと違い、新着がほぼ即時に分かり、
//! 何も起きていない間は通信も発生しない。
//!
//! 通知の中身は解釈しない。「変わった」ことだけを伝え、表示の更新は
//! 通常の一覧取得に任せる (正はあくまでサーバー)。

use std::time::Duration;

use async_imap::extensions::idle::IdleResponse;
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout};

use super::connection::{open_session, ImapSession};
use crate::domain::Account;
use crate::{Error, Result};

const INBOX: &str = "INBOX";
/// IDLE を張り直す間隔。サーバーや NAT が無通信の接続を切る前に更新する。
/// 短めにしているのは、スリープ復帰後に死んだ接続へ早く気付くため。
const IDLE_REFRESH: Duration = Duration::from_secs(5 * 60);
/// IDLE 非対応サーバーでの確認間隔。
const POLL_INTERVAL: Duration = Duration::from_secs(2 * 60);
const RECONNECT_DELAY: Duration = Duration::from_secs(30);
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

/// 監視タスクのハンドル。drop すると監視を止める。
pub struct InboxWatcher {
    task: JoinHandle<()>,
}

impl InboxWatcher {
    /// 監視を開始する。受信トレイに変化があるたびに `on_change` を呼ぶ。
    /// tokio ランタイム上で呼ぶこと。
    pub fn spawn(
        account: Account,
        password: String,
        on_change: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        let task = tokio::spawn(async move {
            loop {
                // 正常終了は無い。エラー (切断など) で戻ってきたら少し待ってつなぎ直す
                let _ = watch(&account, &password, &on_change).await;
                sleep(RECONNECT_DELAY).await;
                // 切断中の変化を取りこぼさないよう、復帰時に一度知らせる
                on_change();
            }
        });
        Self { task }
    }
}

impl Drop for InboxWatcher {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn watch(account: &Account, password: &str, on_change: &(impl Fn() + Sync)) -> Result<()> {
    let session = open_session(&account.imap, &account.username, password).await?;
    watch_session(session, on_change).await
}

/// ログイン済みのセッションで受信トレイを監視し続ける。戻るのはエラー時だけ。
pub async fn watch_session(mut session: ImapSession, on_change: &(impl Fn() + Sync)) -> Result<()> {
    session.select(INBOX).await?;

    if !session.capabilities().await?.has_str("IDLE") {
        loop {
            sleep(POLL_INTERVAL).await;
            session.noop().await?;
            on_change();
        }
    }

    loop {
        let mut idle = session.idle();
        idle.init().await?;
        // `_keep_waiting` を drop すると待機が中断されるので、スコープの間保持する
        let (wait, _keep_waiting) = idle.wait_with_timeout(IDLE_REFRESH);
        let response = wait.await?;
        if matches!(response, IdleResponse::NewData(_)) {
            on_change();
        }
        session = timeout(COMMAND_TIMEOUT, idle.done())
            .await
            .map_err(|_| Error::Timeout)??;
    }
}
