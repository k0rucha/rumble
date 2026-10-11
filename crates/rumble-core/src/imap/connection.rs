//! IMAP 接続の確立と維持。
//!
//! [`ImapConnection`] はアカウントごとに 1 本だけ持つ「操作用」の接続。
//! 接続が切れていれば次の操作の直前につなぎ直すので、呼び出し側は
//! 接続状態を気にせず [`ImapConnection::run`] に操作を渡すだけでよい。

use std::time::{Duration, Instant};

use async_native_tls::{TlsConnector, TlsStream};
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::domain::{Account, Security, ServerEndpoint};
use crate::{Error, Result};

pub type ImapSession = async_imap::Session<TlsStream<TcpStream>>;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
/// 1 操作の上限。大きな添付つきメールの取得もここに収まる必要がある。
const OPERATION_TIMEOUT: Duration = Duration::from_secs(120);
/// これより長く使っていない接続は、使う前に生存確認する。
const LIVENESS_CHECK_AFTER: Duration = Duration::from_secs(30);
const LIVENESS_TIMEOUT: Duration = Duration::from_secs(5);

/// サーバーに接続してログインする。
pub async fn open_session(
    endpoint: &ServerEndpoint,
    username: &str,
    password: &str,
) -> Result<ImapSession> {
    timeout(
        CONNECT_TIMEOUT,
        connect_and_login(endpoint, username, password),
    )
    .await
    .map_err(|_| Error::Timeout)?
}

async fn connect_and_login(
    endpoint: &ServerEndpoint,
    username: &str,
    password: &str,
) -> Result<ImapSession> {
    let host = endpoint.host.as_str();
    let tcp = TcpStream::connect((host, endpoint.port)).await?;
    let tls = TlsConnector::new();

    let client = match endpoint.security {
        Security::Tls => {
            let mut client = async_imap::Client::new(tls.connect(host, tcp).await?);
            read_greeting(&mut client).await?;
            client
        }
        Security::StartTls => {
            let mut plain = async_imap::Client::new(tcp);
            read_greeting(&mut plain).await?;
            plain.run_command_and_check_ok("STARTTLS", None).await?;
            // STARTTLS のあとはサーバーの挨拶が無い
            async_imap::Client::new(tls.connect(host, plain.into_inner()).await?)
        }
    };

    client
        .login(username, password)
        .await
        .map_err(|(error, _client)| match error {
            // 認証の失敗は NO 応答で返る。接続障害とは区別して伝える
            async_imap::error::Error::No(reason) | async_imap::error::Error::Bad(reason) => {
                Error::Auth(reason)
            }
            other => Error::Imap(other),
        })
}

async fn read_greeting<T>(client: &mut async_imap::Client<T>) -> Result<()>
where
    T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    match client.read_response().await? {
        Some(_) => Ok(()),
        None => Err(Error::Imap(async_imap::error::Error::ConnectionLost)),
    }
}

/// アカウント 1 つぶんの操作用接続。
pub struct ImapConnection {
    endpoint: ServerEndpoint,
    username: String,
    password: String,
    session: Option<ImapSession>,
    last_used: Instant,
}

impl ImapConnection {
    /// まだ接続はしない。最初の [`run`](Self::run) で接続する。
    pub fn new(account: &Account, password: String) -> Self {
        Self {
            endpoint: account.imap.clone(),
            username: account.username.clone(),
            password,
            session: None,
            last_used: Instant::now(),
        }
    }

    /// セッションを 1 つ貸し出して `operation` を実行する。
    ///
    /// - 未接続・切断済みなら、先に接続する
    /// - 操作がタイムアウトしたり接続が壊れたりしたら、セッションを捨てる
    ///   (次回の `run` が新しい接続を張る)
    pub async fn run<T>(
        &mut self,
        operation: impl AsyncFnOnce(&mut ImapSession) -> Result<T>,
    ) -> Result<T> {
        let session = self.live_session().await?;
        let result = match timeout(OPERATION_TIMEOUT, operation(session)).await {
            Ok(result) => result,
            Err(_) => Err(Error::Timeout),
        };
        match &result {
            Err(error) if error.is_connection_broken() => self.session = None,
            _ => self.last_used = Instant::now(),
        }
        result
    }

    async fn live_session(&mut self) -> Result<&mut ImapSession> {
        // スリープ復帰後などは、こちらが気付かないまま接続が死んでいることがある。
        // しばらく使っていなければ NOOP で確かめ、だめなら捨ててつなぎ直す。
        if self.last_used.elapsed() > LIVENESS_CHECK_AFTER {
            if let Some(session) = self.session.as_mut() {
                let alive = matches!(timeout(LIVENESS_TIMEOUT, session.noop()).await, Ok(Ok(())));
                if !alive {
                    self.session = None;
                }
            }
        }
        if self.session.is_none() {
            self.session =
                Some(open_session(&self.endpoint, &self.username, &self.password).await?);
        }
        Ok(self.session.as_mut().expect("session was just ensured"))
    }
}
