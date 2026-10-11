//! メール内リンクの扱い。
//!
//! HTML メールはスクリプト禁止の iframe に表示している (`HtmlBody.svelte`)。
//! そこでリンクをクリックすると「アプリのウィンドウ全体を移動する」要求が
//! 届くので、ここで横取りする:
//!
//! - アプリ自身のページ   → そのまま許可
//! - `mailto:`           → rumble の作成画面を開く
//! - `http(s):`          → 既定のブラウザで開く
//! - それ以外            → 何もしない
//!
//! いずれの場合も、外部のページをアプリのウィンドウ内に読み込むことはない。

use tauri::plugin::{Builder, TauriPlugin};
use tauri::{Emitter, Runtime, Url};
use tauri_plugin_opener::OpenerExt;

/// `mailto:` リンクが押されたことをフロントエンドへ知らせるイベント名。
const COMPOSE_EVENT: &str = "compose-to";

pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("external-links")
        .on_navigation(|webview, url| {
            if is_app_page(url) {
                return true;
            }
            match url.scheme() {
                "mailto" => {
                    let _ = webview.emit(COMPOSE_EVENT, url.path());
                }
                "http" | "https" => {
                    let _ = webview.opener().open_url(url.as_str(), None::<&str>);
                }
                _ => {}
            }
            false
        })
        .build()
}

fn is_app_page(url: &Url) -> bool {
    match url.scheme() {
        // iframe の srcdoc (about:srcdoc) や空ページ
        "about" | "tauri" => true,
        "http" | "https" => match url.host_str() {
            // Windows / Android ではアプリのページが http(s)://tauri.localhost になる
            Some("tauri.localhost") => true,
            // 開発時の Vite サーバー
            Some("localhost") => cfg!(debug_assertions),
            _ => false,
        },
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn app_pages_stay_in_the_window() {
        assert!(is_app_page(&url("tauri://localhost/index.html")));
        assert!(is_app_page(&url("https://tauri.localhost/")));
        assert!(is_app_page(&url("about:srcdoc")));
    }

    #[test]
    fn links_in_mail_never_load_inside_the_app() {
        assert!(!is_app_page(&url("https://example.com/")));
        assert!(!is_app_page(&url("https://tauri.localhost.evil.example/")));
        assert!(!is_app_page(&url("mailto:someone@example.com")));
        assert!(!is_app_page(&url("file:///etc/passwd")));
    }
}
