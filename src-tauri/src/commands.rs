//! フロントエンドから `invoke` で呼ばれるコマンド。
//!
//! 1 コマンド = [`MailService`] のメソッド 1 つ。引数を受け渡すだけにして、
//! ここにロジックを置かない (フロント側の対応表は `src/lib/backend.ts`)。

use std::path::PathBuf;
use std::sync::Arc;

use rumble_core::domain::{
    Account, Mailbox, MailboxStatus, MessageBody, MessageFlag, MessagePage, NewAccount,
    OutgoingMessage, SendReport,
};
use rumble_core::{MailService, Result};
use tauri::State;

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
pub async fn save_attachment(
    service: Service<'_>,
    account_id: String,
    mailbox: String,
    uid: u32,
    attachment_index: u32,
    destination: PathBuf,
) -> Result<()> {
    service
        .save_attachment(&account_id, &mailbox, uid, attachment_index, &destination)
        .await
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
