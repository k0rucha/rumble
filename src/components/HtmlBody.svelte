<script lang="ts">
  // HTML メールの表示。
  //
  // 受け取る HTML はバックエンドでサニタイズ済みだが、ここでもう一段隔離する:
  //   - sandbox 付き iframe : スクリプト・フォーム送信・プラグインを一切動かさない
  //   - iframe 内の CSP     : 外部への通信を遮断する (画像は利用者が許可したときだけ)
  //
  // リンクのクリックは「ウィンドウ全体の移動」として Rust 側
  // (src-tauri/src/external_links.rs) に届き、既定のブラウザで開かれる。

  let { html, allowRemoteImages }: { html: string; allowRemoteImages: boolean } = $props();

  const page = $derived(`<!doctype html>
<html>
<head>
<meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; img-src data:${allowRemoteImages ? " https: http:" : ""}">
<base target="_top">
<style>
  /* HTML メールは白背景を前提に作られているので、ダークモードでも紙は白のままにする */
  html { color-scheme: light; background: #fff; }
  body {
    margin: 0;
    padding: 16px 20px;
    color: #1f1d1a;
    font: 14px/1.55 -apple-system, BlinkMacSystemFont, "Hiragino Sans", "Yu Gothic UI", "Segoe UI", sans-serif;
    overflow-wrap: anywhere;
  }
  img { max-width: 100%; height: auto; }
  blockquote { margin: 0 0 0 8px; padding-left: 12px; border-left: 3px solid #ddd; color: #555; }
  pre { white-space: pre-wrap; }
</style>
</head>
<body>${html}</body>
</html>`);
</script>

<iframe
  title="メール本文"
  sandbox="allow-top-navigation-by-user-activation"
  srcdoc={page}
></iframe>

<style>
  iframe {
    flex: 1;
    width: 100%;
    min-height: 0;
    border: none;
    background: #fff;
  }
</style>
