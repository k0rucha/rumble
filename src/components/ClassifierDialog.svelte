<script lang="ts">
  // 自動分類の設定画面。
  // メールの一部を外部サービス (TypeSafe) に送る機能なので、何が送られるかを
  // 明示し、利用者が API キーを入れたときだけ有効になる。
  import { store } from "../lib/store.svelte";
  import Dialog from "./Dialog.svelte";

  let apiKey = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);

  async function enable(event: SubmitEvent) {
    event.preventDefault();
    if (apiKey.trim() === "" || busy) return;
    busy = true;
    error = null;
    try {
      await store.enableClassifier(apiKey);
    } catch (cause) {
      error = String(cause);
    } finally {
      busy = false;
    }
  }
</script>

<Dialog title="自動分類" onclose={() => (store.classifierDialogOpen = false)}>
  <form id="classifier-form" onsubmit={enable}>
    <p>
      届いたメールを「個人・通知・取引・宣伝」に自動で分けます。判断には TypeSafe の分類モデル
      <strong>Jev</strong> を使います。
    </p>
    <ul>
      <li>
        TypeSafe に送るのは<strong>差出人・宛先・件名</strong>と、一斉配信かどうかの情報だけです。
        <strong>本文と添付は送りません。</strong>
      </li>
      <li>
        分類結果はメールサーバー上のメールに印 (IMAP キーワード) として付けます。この端末には保存せず、
        ほかの端末の rumble からも同じ分類が見えます。Gmail などではラベルとして見える場合があります。
      </li>
      <li>Jev は英語を中心に学習しており、日本語の件名では精度が落ちることがあります。</li>
    </ul>

    {#if store.classifierEnabled}
      <p class="status">自動分類は<strong>有効</strong>です。</p>
    {:else}
      <label>
        <span>API キー</span>
        <!-- svelte-ignore a11y_autofocus : 開いたらすぐ入力を始められるように -->
        <input type="password" bind:value={apiKey} autofocus autocapitalize="off" spellcheck="false" />
      </label>
      <p class="muted hint">
        TypeSafe で発行した API キーを入力してください。キーは OS のキーチェーン
        (Android ではアプリ専用領域) に保存されます。
      </p>
    {/if}

    {#if error}
      <p class="error selectable">{error}</p>
    {/if}
  </form>

  {#snippet footer()}
    {#if store.classifierEnabled}
      <button onclick={() => store.disableClassifier()}>無効にしてキーを削除</button>
    {:else}
      <button class="primary" type="submit" form="classifier-form" disabled={apiKey.trim() === "" || busy}>
        {busy ? "確認中…" : "有効にする"}
      </button>
    {/if}
    <button type="button" onclick={() => (store.classifierDialogOpen = false)}>閉じる</button>
  {/snippet}
</Dialog>

<style>
  form {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  p {
    margin: 0;
  }

  ul {
    margin: 0;
    padding-left: 20px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  label {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  label span {
    width: 72px;
    color: var(--text-muted);
  }

  label input {
    flex: 1;
    min-width: 0;
  }

  .hint {
    font-size: 12px;
  }

  .status {
    padding: 8px 10px;
    border-radius: 6px;
    background: var(--accent-soft);
  }

  .error {
    padding: 8px 10px;
    border-radius: 6px;
    background: var(--accent-soft);
    color: var(--danger);
    overflow-wrap: anywhere;
  }
</style>
