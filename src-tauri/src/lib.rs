//! rumble の Tauri シェル。
//!
//! ここは「画面と [`rumble_core`] をつなぐ配線」だけを担当する:
//!
//! - [`commands`]       : フロントエンドから呼べる関数 (中身は MailService へ丸投げ)
//! - [`external_links`] : メール内のリンクをアプリ内で開かず、既定のブラウザへ回す
//!
//! メールに関する判断やロジックはここには書かない。

mod commands;
mod external_links;

use std::sync::Arc;

use rumble_core::storage::{JsonAccountStore, SecretStore};
use rumble_core::MailService;
use tauri::{Emitter, Manager};

/// サーバー側の変化をフロントエンドへ知らせるイベント名。
const MAIL_EVENT: &str = "mail-event";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(external_links::plugin())
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let handle = app.handle().clone();

            let service = Arc::new(MailService::new(
                Box::new(JsonAccountStore::new(config_dir.join("accounts.json"))),
                secret_store(app)?,
                Arc::new(move |event| {
                    let _ = handle.emit(MAIL_EVENT, event);
                }),
            )?);
            app.manage(service.clone());

            // 監視タスクは tokio ランタイム上で起動する必要がある
            tauri::async_runtime::spawn(async move { service.start_watchers() });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_accounts,
            commands::add_account,
            commands::remove_account,
            commands::list_mailboxes,
            commands::mailbox_status,
            commands::list_messages,
            commands::search_messages,
            commands::get_message,
            commands::verify_sender,
            commands::message_source,
            commands::save_attachment,
            commands::set_flag,
            commands::move_messages,
            commands::delete_messages,
            commands::send_message,
            commands::classifier_enabled,
            commands::enable_classifier,
            commands::disable_classifier,
            commands::classify_messages,
            commands::list_by_category,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start rumble");
}

/// パスワードの保管先。デスクトップでは OS のキーチェーン。
#[cfg(not(target_os = "android"))]
fn secret_store(app: &tauri::App) -> tauri::Result<Box<dyn SecretStore>> {
    use rumble_core::storage::KeychainSecretStore;
    Ok(Box::new(KeychainSecretStore::new(
        app.config().identifier.clone(),
    )))
}

/// Android には使える資格情報ストアが無いので、他のアプリから読めない
/// アプリ専用領域のファイルに置く。
#[cfg(target_os = "android")]
fn secret_store(app: &tauri::App) -> tauri::Result<Box<dyn SecretStore>> {
    use rumble_core::storage::FileSecretStore;
    Ok(Box::new(FileSecretStore::new(
        app.path().app_data_dir()?.join("secrets.json"),
    )))
}
