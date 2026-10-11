//! フロントエンドから `invoke` で呼ばれるコマンド。
//!
//! 1 コマンド = [`MailService`] のメソッド 1 つ。引数を受け渡すだけにして、
//! ここにロジックを置かない (フロント側の対応表は `src/lib/backend.ts`)。

use std::io::Write;
use std::sync::Arc;

use rumble_core::domain::{
    Account, Category, Mailbox, MailboxStatus, MessageBody, MessageFlag, MessagePage,
    MessageSource, NewAccount, OutgoingMessage, SendReport, SenderAuth,
};
use rumble_core::{MailService, Result};
use tauri::{AppHandle, State};
use tauri_plugin_fs::{FilePath, FsExt, OpenOptions};

type Service<'a> = State<'a, Arc<MailService>>;

#[tauri::command]
pub fn list_accounts(service: Service<'_>) -> Vec<Account> {
    service.list_accounts()
}

#[tauri::command]
pub async fn add_account(service: Service<'_>, account: NewAccount) -> Result<Account> {
    service.add_account(account).await
}

#[tauri::command]
pub fn remove_account(service: Service<'_>, account_id: String) -> Result<()> {
    service.remove_account(&account_id)
}

#[tauri::command]
pub async fn list_mailboxes(service: Service<'_>, account_id: String) -> Result<Vec<Mailbox>> {
    service.list_mailboxes(&account_id).await
}

#[tauri::command]
pub async fn mailbox_status(
    service: Service<'_>,
    account_id: String,
    mailbox: String,
) -> Result<MailboxStatus> {
    service.mailbox_status(&account_id, &mailbox).await
}

#[tauri::command]
pub async fn list_messages(
    service: Service<'_>,
    account_id: String,
    mailbox: String,
    offset: u32,
    limit: u32,
) -> Result<MessagePage> {
    service
        .list_messages(&account_id, &mailbox, offset, limit)
        .await
}

#[tauri::command]
pub async fn search_messages(
    service: Service<'_>,
    account_id: String,
    mailbox: String,
    query: String,
) -> Result<MessagePage> {
    service.search_messages(&account_id, &mailbox, &query).await
}

#[tauri::command]
pub async fn get_message(
    service: Service<'_>,
    account_id: String,
    mailbox: String,
    uid: u32,
) -> Result<MessageBody> {
    service.get_message(&account_id, &mailbox, uid).await
}

#[tauri::command]
pub async fn verify_sender(
    service: Service<'_>,
    account_id: String,
    mailbox: String,
    uid: u32,
) -> Result<SenderAuth> {
    service.verify_sender(&account_id, &mailbox, uid).await
}

#[tauri::command]
pub async fn message_source(
    service: Service<'_>,
    account_id: String,
    mailbox: String,
    uid: u32,
) -> Result<MessageSource> {
    service.message_source(&account_id, &mailbox, uid).await
}

/// 添付ファイルを、利用者が保存ダイアログで選んだ場所へ書き出す。
///
/// rumble がメールの中身をディスクに書くのはここだけ。Android の保存ダイアログは
/// パスではなく `content://` の URI を返すので、どちらも扱える fs プラグイン経由で書く。
#[tauri::command]
pub async fn save_attachment(
    app: AppHandle,
    service: Service<'_>,
    account_id: String,
    mailbox: String,
    uid: u32,
    attachment_index: u32,
    destination: FilePath,
) -> Result<()> {
    let (_filename, bytes) = service
        .attachment(&account_id, &mailbox, uid, attachment_index)
        .await?;
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    app.fs().open(destination, options)?.write_all(&bytes)?;
    Ok(())
}

#[tauri::command]
pub async fn set_flag(
    service: Service<'_>,
    account_id: String,
    mailbox: String,
    uids: Vec<u32>,
    flag: MessageFlag,
    enabled: bool,
) -> Result<()> {
    service
        .set_flag(&account_id, &mailbox, &uids, flag, enabled)
        .await
}

#[tauri::command]
pub async fn move_messages(
    service: Service<'_>,
    account_id: String,
    mailbox: String,
    uids: Vec<u32>,
    destination: String,
) -> Result<()> {
    service
        .move_messages(&account_id, &mailbox, &uids, &destination)
        .await
}

#[tauri::command]
pub async fn delete_messages(
    service: Service<'_>,
    account_id: String,
    mailbox: String,
    uids: Vec<u32>,
) -> Result<()> {
    service.delete_messages(&account_id, &mailbox, &uids).await
}

#[tauri::command]
pub async fn send_message(
    service: Service<'_>,
    account_id: String,
    message: OutgoingMessage,
) -> Result<SendReport> {
    service.send_message(&account_id, message).await
}

#[tauri::command]
pub fn classifier_enabled(service: Service<'_>) -> bool {
    service.classifier_enabled()
}

#[tauri::command]
pub async fn enable_classifier(service: Service<'_>, api_key: String) -> Result<()> {
    service.enable_classifier(&api_key).await
}

#[tauri::command]
pub fn disable_classifier(service: Service<'_>) -> Result<()> {
    service.disable_classifier()
}

/// 戻り値は `[uid, 区分]` の組の配列。
#[tauri::command]
pub async fn classify_messages(
    service: Service<'_>,
    account_id: String,
    mailbox: String,
    uids: Vec<u32>,
) -> Result<Vec<(u32, Category)>> {
    service
        .classify_messages(&account_id, &mailbox, &uids)
        .await
}

#[tauri::command]
pub async fn list_by_category(
    service: Service<'_>,
    account_id: String,
    mailbox: String,
    category: Category,
) -> Result<MessagePage> {
    service
        .list_by_category(&account_id, &mailbox, category)
        .await
}
